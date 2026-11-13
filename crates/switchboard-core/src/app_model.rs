use crate::{
    model::{AccountRecord, AuthState, StoreFile},
    quota::QuotaResult,
    store::{CredentialStore, LoadStore, Store},
    target::{emails_match, TargetAdapter, TargetFuture, TargetSnapshot},
    update::AppRelease,
};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use uuid::Uuid;

/// Side effects supplied by the application shell. Long-running operations must
/// be cancellable when their returned future is dropped.
pub trait AppServices: Send + Sync + 'static {
    fn login(&self) -> TargetFuture<'_, Result<TargetSnapshot, String>>;
    fn cancel_login(&self);
    fn quota<'a>(&'a self, snapshot: &'a TargetSnapshot) -> TargetFuture<'a, QuotaResult>;
    fn launch_at_login(&self) -> Result<bool, String>;
    fn set_launch_at_login(&self, enabled: bool) -> Result<(), String>;
    fn check_updates(&self, force: bool) -> TargetFuture<'_, Result<Option<AppRelease>, String>>;
    fn start_update(
        &self,
        release: AppRelease,
        progress: mpsc::UnboundedSender<(u64, Option<u64>)>,
    ) -> TargetFuture<'_, Result<String, String>>;
    fn install_update<'a>(&'a self, prepared: &'a str) -> TargetFuture<'a, Result<(), String>>;
    fn open_update_page(&self, url: &str) -> Result<(), String>;
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Feedback {
    pub text: String,
    pub kind: FeedbackKind,
    pub caption: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackKind {
    Success,
    Loading,
    Warning,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UpdateState {
    Idle,
    Available {
        release: AppRelease,
        silenced: bool,
    },
    Downloading {
        release: AppRelease,
        percent: u8,
        received: u64,
        total: Option<u64>,
    },
    Ready {
        release: AppRelease,
        prepared: String,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewState {
    pub accounts: Vec<AccountRecord>,
    pub live_email: Option<String>,
    pub unregistered_live_email: Option<String>,
    pub switching_to: Option<Uuid>,
    pub refreshing: bool,
    pub adding_account: bool,
    pub feedback: Option<Feedback>,
    pub update: UpdateState,
    pub launch_at_login: bool,
    pub version: String,
    pub persistence_blocked: bool,
    pub demo: bool,
}

pub enum Command {
    SyncIdentity,
    AddAccount,
    CancelAddAccount,
    ImportCurrentLogin,
    SwitchTo(Uuid),
    RemoveAccount(Uuid),
    RefreshQuotas,
    SetLaunchAtLogin(bool),
    CheckUpdates(bool),
    StartUpdate,
    PostponeUpdate,
    InstallUpdate,
    OpenUpdatePage,
    Flush,
}

enum Message {
    Command(Command),
    Persist(u64),
    ClearFeedback(u64),
    LoginFinished {
        generation: u64,
        result: Result<TargetSnapshot, String>,
    },
    FlushAndReply(oneshot::Sender<()>),
    SwitchFinished {
        id: Uuid,
        result: SwitchResult,
        new_login: bool,
    },
    QuotasFinished(Vec<(Uuid, QuotaResult)>),
    SoftVerify {
        expected: String,
        generation: u64,
    },
    UpdateChecked {
        force: bool,
        result: Result<Option<AppRelease>, String>,
    },
    UpdateProgress {
        received: u64,
        total: Option<u64>,
    },
    UpdateDownloaded {
        release: AppRelease,
        result: Result<String, String>,
    },
    UpdateInstalled(Result<(), String>),
}

enum SwitchResult {
    Completed {
        launch_error: Option<String>,
        relaunched: bool,
    },
    Failed(String),
}

#[derive(Clone)]
pub struct AppHandle {
    sender: mpsc::UnboundedSender<Message>,
    state: watch::Receiver<ViewState>,
}
impl AppHandle {
    pub fn send(&self, command: Command) {
        let _ = self.sender.send(Message::Command(command));
    }
    pub fn state(&self) -> ViewState {
        self.state.borrow().clone()
    }
    pub async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.sender.send(Message::FlushAndReply(tx)).is_ok() {
            let _ = rx.await;
        }
    }
    pub fn subscribe(&self) -> watch::Receiver<ViewState> {
        self.state.clone()
    }
}

struct Model {
    state: ViewState,
    target: Arc<dyn TargetAdapter>,
    services: Arc<dyn AppServices>,
    credentials: Arc<dyn CredentialStore>,
    store: Store,
    tx: mpsc::UnboundedSender<Message>,
    state_tx: watch::Sender<ViewState>,
    persist_generation: u64,
    feedback_generation: u64,
    verify_generation: u64,
    login_generation: u64,
    login_task: Option<JoinHandle<()>>,
    update_task: Option<JoinHandle<()>>,
}

pub fn start(
    target: Arc<dyn TargetAdapter>,
    services: Arc<dyn AppServices>,
    credentials: Arc<dyn CredentialStore>,
    store: Store,
    version: String,
    demo: bool,
) -> AppHandle {
    let (accounts, blocked, notice) = if demo {
        (demo_accounts(), false, None)
    } else {
        match store.load() {
            LoadStore::Ready(file) => (file.accounts, false, None),
            LoadStore::Blocked(message) => (Vec::new(), true, Some(message)),
        }
    };
    let state = ViewState {
        accounts,
        live_email: None,
        unregistered_live_email: None,
        switching_to: None,
        refreshing: false,
        adding_account: false,
        feedback: notice.map(|text| Feedback {
            text: text.into(),
            kind: FeedbackKind::Warning,
            caption: None,
        }),
        update: UpdateState::Idle,
        launch_at_login: if demo {
            false
        } else {
            services.launch_at_login().unwrap_or(false)
        },
        version,
        persistence_blocked: blocked,
        demo,
    };
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (state_tx, state_rx) = watch::channel(state.clone());
    let mut model = Model {
        state,
        target,
        services,
        credentials,
        store,
        tx: tx.clone(),
        state_tx,
        persist_generation: 0,
        feedback_generation: 0,
        verify_generation: 0,
        login_generation: 0,
        login_task: None,
        update_task: None,
    };
    tokio::spawn(async move {
        if !model.state.demo {
            model.sync_identity();
            model.publish();
        }
        while let Some(message) = rx.recv().await {
            model.handle(message);
            model.publish();
        }
    });
    AppHandle {
        sender: tx,
        state: state_rx,
    }
}

fn demo_accounts() -> Vec<AccountRecord> {
    [
        ("alex.chen@gmail.com", "Pro 方案", true, 1),
        ("jamie.work@outlook.com", "免费方案", false, 2),
        ("alex.creative@icloud.com", "团队方案", false, 4),
    ]
    .into_iter()
    .map(|(email, plan, current, used)| AccountRecord {
        id: Uuid::new_v4(),
        email: email.into(),
        plan: Some(plan.into()),
        is_current: current,
        used_days: Some(used),
        total_days: Some(5),
        last_quota_update: Some(now_swift()),
        last_error: None,
        auth_state: AuthState::Ok,
    })
    .collect()
}
fn now_swift() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        - 978_307_200.0
}
fn save_snapshot(
    credentials: &dyn CredentialStore,
    id: Uuid,
    snapshot: &TargetSnapshot,
) -> Result<(), String> {
    let data = serde_json::to_vec(snapshot).map_err(|e| e.to_string())?;
    credentials
        .save(&id.to_string(), &data)
        .map_err(|e| e.to_string())
}
enum SnapshotLoadError {
    Missing,
    Other(String),
}
impl SnapshotLoadError {
    fn message(self) -> String {
        match self {
            Self::Missing => "该账号的登录凭据缺失，请重新添加".into(),
            Self::Other(message) => message,
        }
    }
}
fn load_snapshot(
    credentials: &dyn CredentialStore,
    id: Uuid,
) -> Result<TargetSnapshot, SnapshotLoadError> {
    let data = credentials
        .load(&id.to_string())
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput => {
                SnapshotLoadError::Missing
            }
            _ => SnapshotLoadError::Other(error.to_string()),
        })?;
    serde_json::from_slice(&data).map_err(|_| SnapshotLoadError::Missing)
}

impl Model {
    fn publish(&mut self) {
        self.state.unregistered_live_email = self
            .state
            .live_email
            .as_ref()
            .filter(|email| {
                !self
                    .state
                    .accounts
                    .iter()
                    .any(|a| emails_match(&a.email, email))
            })
            .cloned();
        self.state_tx.send_replace(self.state.clone());
    }
    fn feedback(
        &mut self,
        text: String,
        kind: FeedbackKind,
        caption: Option<String>,
        persistent: bool,
    ) {
        self.feedback_generation += 1;
        self.state.feedback = Some(Feedback {
            text: text.clone(),
            kind,
            caption,
        });
        if !persistent {
            let generation = self.feedback_generation;
            let tx = self.tx.clone();
            let seconds = (text.chars().count() / 10 + 4).clamp(6, 20) as u64;
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                let _ = tx.send(Message::ClearFeedback(generation));
            });
        }
    }
    fn schedule_persist(&mut self) {
        if self.state.demo || self.state.persistence_blocked {
            return;
        }
        self.persist_generation += 1;
        let generation = self.persist_generation;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            let _ = tx.send(Message::Persist(generation));
        });
    }
    fn flush(&mut self) {
        self.persist_generation += 1;
        if self.state.demo || self.state.persistence_blocked {
            return;
        }
        if let Err(e) = self.store.write(&StoreFile {
            store_version: 1,
            accounts: self.state.accounts.clone(),
        }) {
            if self.state.feedback.is_none() {
                self.feedback(
                    format!("保存设置失败：{e}"),
                    FeedbackKind::Warning,
                    None,
                    false,
                );
            }
        }
    }
    fn writable(&mut self) -> bool {
        if !self.state.persistence_blocked {
            return true;
        }
        self.feedback(
            "账号列表文件异常，暂不能修改账号列表。请检查 store.json 后重启应用".into(),
            FeedbackKind::Warning,
            None,
            false,
        );
        false
    }
    fn sync_identity(&mut self) {
        if self.state.demo {
            return;
        }
        self.state.live_email = self.target.live_identity();
        let mut changed = false;
        for record in &mut self.state.accounts {
            let current = self
                .state
                .live_email
                .as_ref()
                .is_some_and(|email| emails_match(email, &record.email));
            if record.is_current != current {
                record.is_current = current;
                changed = true;
            }
        }
        if changed {
            self.schedule_persist();
        }
    }
    fn busy(&self) -> bool {
        self.state.adding_account || self.state.switching_to.is_some() || self.state.refreshing
    }
    fn demo_blocked(&mut self, text: &str) -> bool {
        if self.state.demo {
            self.feedback(text.into(), FeedbackKind::Warning, None, false);
            true
        } else {
            false
        }
    }
    fn begin_login(&mut self) {
        if self.demo_blocked("演示模式不操作真实账号") {
            return;
        }
        if self.state.adding_account {
            self.cancel_login();
            return;
        }
        if !self.writable() || self.busy() {
            return;
        }
        self.state.adding_account = true;
        self.feedback(
            "请在网页登录要添加的账号".into(),
            FeedbackKind::Loading,
            None,
            true,
        );
        self.login_generation += 1;
        let generation = self.login_generation;
        let services = self.services.clone();
        let tx = self.tx.clone();
        self.login_task = Some(tokio::spawn(async move {
            let result =
                match tokio::time::timeout(Duration::from_secs(300), services.login()).await {
                    Ok(result) => result,
                    Err(_) => {
                        services.cancel_login();
                        Err("网页登录已取消或超时，原账号保持不变".into())
                    }
                };
            let _ = tx.send(Message::LoginFinished { generation, result });
        }));
    }
    fn cancel_login(&mut self) {
        if self.state.adding_account && self.state.switching_to.is_none() {
            self.login_generation += 1;
            if let Some(task) = self.login_task.take() {
                task.abort();
            }
            self.services.cancel_login();
            self.state.adding_account = false;
            self.feedback(
                "网页登录已取消或超时，原账号保持不变".into(),
                FeedbackKind::Warning,
                None,
                false,
            );
        }
    }
    fn new_record(email: String) -> AccountRecord {
        AccountRecord {
            id: Uuid::new_v4(),
            email,
            plan: None,
            is_current: false,
            used_days: None,
            total_days: None,
            last_quota_update: None,
            last_error: None,
            auth_state: AuthState::Ok,
        }
    }
    fn save_login(&mut self, snapshot: TargetSnapshot, activate: bool) {
        let found = self
            .state
            .accounts
            .iter()
            .position(|record| emails_match(&record.email, &snapshot.email));
        let record = found
            .map(|i| self.state.accounts[i].clone())
            .unwrap_or_else(|| Self::new_record(snapshot.email.clone()));
        if let Err(error) = save_snapshot(self.credentials.as_ref(), record.id, &snapshot) {
            self.state.adding_account = false;
            self.feedback(
                format!("添加失败：{error}"),
                FeedbackKind::Warning,
                None,
                false,
            );
            return;
        }
        if let Some(i) = found {
            self.state.accounts[i].auth_state = AuthState::Ok;
            self.state.accounts[i].last_error = None;
        } else {
            self.state.accounts.push(record.clone());
        }
        self.flush();
        if activate {
            self.begin_switch(record.id, true);
        } else {
            self.sync_identity();
            self.flush();
            let text = if found.is_some() {
                format!("账号 {} 的登录状态已更新", snapshot.email)
            } else {
                format!("已添加账号 {}", snapshot.email)
            };
            self.feedback(text, FeedbackKind::Success, None, false);
            self.begin_refresh();
        }
    }
    fn begin_switch(&mut self, id: Uuid, new_login: bool) {
        self.verify_generation += 1;
        if self.state.adding_account && !new_login {
            return;
        }
        if self.demo_blocked("演示模式不操作真实账号") {
            return;
        }
        if self.state.switching_to.is_some() {
            return;
        }
        if self.state.refreshing {
            self.feedback(
                "正在刷新额度，请稍后再切换".into(),
                FeedbackKind::Warning,
                None,
                false,
            );
            return;
        }
        self.sync_identity();
        let Some(record) = self.state.accounts.iter().find(|a| a.id == id).cloned() else {
            if new_login {
                self.state.adding_account = false;
            }
            return;
        };
        if record.is_current && !new_login {
            return;
        }
        self.state.switching_to = Some(id);
        let target = self.target.clone();
        let credentials = self.credentials.clone();
        let accounts = self.state.accounts.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = switch_account(
                target.as_ref(),
                credentials.as_ref(),
                &accounts,
                &record,
                new_login,
            )
            .await;
            let _ = tx.send(Message::SwitchFinished {
                id,
                result,
                new_login,
            });
        });
    }
    fn begin_refresh(&mut self) {
        if self.demo_blocked("演示模式不请求真实额度") || self.busy() {
            return;
        }
        self.state.refreshing = true;
        self.sync_identity();
        self.feedback("正在刷新额度…".into(), FeedbackKind::Loading, None, true);
        let jobs: Vec<_> = self
            .state
            .accounts
            .iter()
            .map(|record| {
                let snapshot = match self.target.capture_snapshot() {
                    Ok(live) if emails_match(&live.email, &record.email) => {
                        let _ = save_snapshot(self.credentials.as_ref(), record.id, &live);
                        Ok(live)
                    }
                    _ => load_snapshot(self.credentials.as_ref(), record.id),
                };
                (record.id, snapshot)
            })
            .collect();
        let services = self.services.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let results = stream::iter(jobs)
                .map(|(id, snapshot)| {
                    let services = services.clone();
                    async move {
                        (
                            id,
                            match snapshot {
                                Ok(snapshot) => services.quota(&snapshot).await,
                                Err(SnapshotLoadError::Missing) => QuotaResult::Expired,
                                Err(SnapshotLoadError::Other(message)) => {
                                    QuotaResult::Failure(message)
                                }
                            },
                        )
                    }
                })
                .buffer_unordered(3)
                .collect()
                .await;
            let _ = tx.send(Message::QuotasFinished(results));
        });
    }
    fn finish_refresh(&mut self, results: Vec<(Uuid, QuotaResult)>) {
        let mut failures = 0;
        for (id, result) in results {
            if matches!(result, QuotaResult::Expired | QuotaResult::Failure(_)) {
                failures += 1;
            }
            let Some(record) = self.state.accounts.iter_mut().find(|a| a.id == id) else {
                continue;
            };
            match result {
                QuotaResult::Quota { used, total, plan } => {
                    record.used_days = Some(used);
                    record.total_days = Some(total);
                    if plan.is_some() {
                        record.plan = plan;
                    }
                    record.last_quota_update = Some(now_swift());
                    record.last_error = None;
                    record.auth_state = AuthState::Ok;
                }
                QuotaResult::Unsupported => {
                    record.used_days = None;
                    record.total_days = None;
                    record.last_quota_update = Some(now_swift());
                    record.last_error = None;
                    record.auth_state = AuthState::Ok;
                }
                QuotaResult::Expired => {
                    record.auth_state = AuthState::Expired;
                    record.last_error = Some("登录已过期，请重新登录该账号".into());
                }
                QuotaResult::Failure(message) => record.last_error = Some(message),
            }
        }
        self.state.refreshing = false;
        self.flush();
        let count = self.state.accounts.len();
        let (message, kind, caption) = if count == 0 {
            (
                "还没有可刷新的账号".to_string(),
                FeedbackKind::Warning,
                None,
            )
        } else if failures == 0 {
            (
                "额度已刷新".to_string(),
                FeedbackKind::Success,
                Some("刚刚更新".to_string()),
            )
        } else if failures < count {
            (
                format!("额度已刷新，{failures} 个账号更新失败"),
                FeedbackKind::Warning,
                None,
            )
        } else {
            ("额度更新失败".to_string(), FeedbackKind::Warning, None)
        };
        self.feedback(message, kind, caption, false);
    }
    fn remove(&mut self, id: Uuid) {
        if self.busy() {
            self.feedback(
                "有操作正在进行，请稍后再移除".into(),
                FeedbackKind::Warning,
                None,
                false,
            );
            return;
        }
        if self.demo_blocked("演示模式不操作真实账号") || !self.writable() {
            return;
        }
        self.sync_identity();
        let Some(record) = self
            .state
            .accounts
            .iter()
            .find(|a| a.id == id && !a.is_current)
            .cloned()
        else {
            return;
        };
        match self.credentials.delete(&id.to_string()) {
            Ok(()) => {
                self.state.accounts.retain(|a| a.id != id);
                self.flush();
                self.feedback(
                    format!("已移除 {}", record.email),
                    FeedbackKind::Success,
                    None,
                    false,
                );
            }
            Err(e) => self.feedback(format!("移除失败：{e}"), FeedbackKind::Warning, None, false),
        }
    }
    fn begin_update_check(&mut self, force: bool) {
        if self.state.demo
            || matches!(
                self.state.update,
                UpdateState::Downloading { .. } | UpdateState::Ready { .. }
            )
        {
            return;
        }
        if let Some(task) = self.update_task.take() {
            task.abort();
        }
        let services = self.services.clone();
        let tx = self.tx.clone();
        self.update_task = Some(tokio::spawn(async move {
            let result = services.check_updates(force).await;
            let _ = tx.send(Message::UpdateChecked { force, result });
        }));
    }
    fn begin_update(&mut self) {
        let UpdateState::Available { release, .. } = &self.state.update else {
            return;
        };
        let release = release.clone();
        if let Some(task) = self.update_task.take() {
            task.abort();
        }
        self.state.update = UpdateState::Downloading {
            release: release.clone(),
            percent: 0,
            received: 0,
            total: None,
        };
        let services = self.services.clone();
        let tx = self.tx.clone();
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();
        tokio::spawn(async move {
            while let Some((received, total)) = progress_rx.recv().await {
                let _ = event_tx.send(Message::UpdateProgress { received, total });
            }
        });
        self.update_task = Some(tokio::spawn(async move {
            let result = services.start_update(release.clone(), progress_tx).await;
            let _ = tx.send(Message::UpdateDownloaded { release, result });
        }));
    }
    fn handle(&mut self, message: Message) {
        match message {
            Message::Command(command) => match command {
                Command::SyncIdentity => self.sync_identity(),
                Command::AddAccount => self.begin_login(),
                Command::CancelAddAccount => self.cancel_login(),
                Command::ImportCurrentLogin => {
                    if !self.busy()
                        && !self.demo_blocked("演示模式不操作真实账号")
                        && self.writable()
                    {
                        match self.target.capture_snapshot() {
                            Ok(snapshot) => self.save_login(snapshot, false),
                            Err(e) => self.feedback(
                                format!("添加失败：{e}"),
                                FeedbackKind::Warning,
                                None,
                                false,
                            ),
                        }
                    }
                }
                Command::SwitchTo(id) => self.begin_switch(id, false),
                Command::RemoveAccount(id) => self.remove(id),
                Command::RefreshQuotas => self.begin_refresh(),
                Command::SetLaunchAtLogin(enabled) => {
                    if self.state.demo {
                        self.state.launch_at_login = enabled;
                    } else {
                        match self.services.set_launch_at_login(enabled) {
                            Ok(()) => self.state.launch_at_login = enabled,
                            Err(e) => {
                                self.state.launch_at_login =
                                    self.services.launch_at_login().unwrap_or(false);
                                self.feedback(
                                    format!("设置开机启动失败：{e}"),
                                    FeedbackKind::Warning,
                                    None,
                                    false,
                                );
                            }
                        }
                    }
                }
                Command::CheckUpdates(force) => self.begin_update_check(force),
                Command::StartUpdate => self.begin_update(),
                Command::PostponeUpdate => {
                    if let UpdateState::Available { silenced, .. } = &mut self.state.update {
                        *silenced = true;
                    }
                }
                Command::InstallUpdate => {
                    if let UpdateState::Ready { prepared, .. } = &self.state.update {
                        let prepared = prepared.clone();
                        let services = self.services.clone();
                        let tx = self.tx.clone();
                        tokio::spawn(async move {
                            let result = services.install_update(&prepared).await;
                            let _ = tx.send(Message::UpdateInstalled(result));
                        });
                    }
                }
                Command::OpenUpdatePage => {
                    let release = match &self.state.update {
                        UpdateState::Available { release, .. }
                        | UpdateState::Downloading { release, .. }
                        | UpdateState::Ready { release, .. } => Some(release.clone()),
                        _ => None,
                    };
                    if let Some(release) = release {
                        if let Err(e) = self.services.open_update_page(&release.page_url) {
                            self.feedback(
                                format!("打开更新页面失败：{e}"),
                                FeedbackKind::Warning,
                                None,
                                false,
                            );
                        }
                    }
                }
                Command::Flush => self.flush(),
            },
            Message::Persist(generation) => {
                if generation == self.persist_generation {
                    self.flush();
                }
            }
            Message::ClearFeedback(generation) => {
                if generation == self.feedback_generation {
                    self.state.feedback = None;
                }
            }
            Message::FlushAndReply(reply) => {
                self.flush();
                let _ = reply.send(());
            }
            Message::LoginFinished { generation, result } => {
                if generation != self.login_generation {
                    return;
                }
                self.login_task = None;
                if !self.state.adding_account {
                    return;
                }
                match result {
                    Ok(snapshot) => self.save_login(snapshot, true),
                    Err(e) => {
                        self.state.adding_account = false;
                        let message = if e == "网页登录已取消或超时，原账号保持不变"
                        {
                            e
                        } else {
                            format!("添加失败：{e}")
                        };
                        self.feedback(message, FeedbackKind::Warning, None, false);
                    }
                }
            }
            Message::SwitchFinished {
                id,
                result,
                new_login,
            } => {
                self.state.switching_to = None;
                self.sync_identity();
                if new_login {
                    self.state.adding_account = false;
                }
                match result {
                    SwitchResult::Completed {
                        launch_error,
                        relaunched,
                    } => {
                        let email = self
                            .state
                            .accounts
                            .iter()
                            .find(|a| a.id == id)
                            .map(|a| a.email.clone())
                            .unwrap_or_default();
                        if !self
                            .state
                            .live_email
                            .as_ref()
                            .is_some_and(|live| emails_match(live, &email))
                        {
                            let message = format!(
                                "切换后身份校验失败（期望 {}，实际 {}）",
                                email,
                                self.state.live_email.as_deref().unwrap_or("无")
                            );
                            self.feedback(
                                if new_login {
                                    format!("账号已保存，但切换失败：{message}")
                                } else {
                                    format!("切换失败：{message}")
                                },
                                FeedbackKind::Warning,
                                None,
                                false,
                            );
                            return;
                        }
                        if let Some(record) = self.state.accounts.iter_mut().find(|a| a.id == id) {
                            record.auth_state = AuthState::Ok;
                            record.last_error = None;
                        }
                        self.schedule_persist();
                        if let Some(error) = launch_error {
                            self.feedback(
                                format!("已切换到 {email}，但 Pen 启动失败，请手动打开：{error}"),
                                FeedbackKind::Warning,
                                None,
                                false,
                            );
                        } else {
                            self.feedback(
                                format!("已切换到 {email}"),
                                FeedbackKind::Success,
                                None,
                                false,
                            );
                            if relaunched {
                                let generation = self.verify_generation;
                                let tx = self.tx.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_secs(4)).await;
                                    let _ = tx.send(Message::SoftVerify {
                                        expected: email,
                                        generation,
                                    });
                                });
                            }
                        }
                        if new_login {
                            self.begin_refresh();
                        }
                    }
                    SwitchResult::Failed(message) => self.feedback(
                        if new_login {
                            format!("账号已保存，但切换失败：{message}")
                        } else {
                            format!("切换失败：{message}")
                        },
                        FeedbackKind::Warning,
                        None,
                        false,
                    ),
                }
            }
            Message::QuotasFinished(results) => self.finish_refresh(results),
            Message::SoftVerify {
                expected,
                generation,
            } => {
                if generation == self.verify_generation {
                    self.sync_identity();
                    if !self
                        .state
                        .live_email
                        .as_ref()
                        .is_some_and(|email| emails_match(email, &expected))
                    {
                        self.feedback(
                            "Pen 启动后登录身份已变化，请确认当前账号".into(),
                            FeedbackKind::Warning,
                            None,
                            false,
                        );
                    }
                }
            }
            Message::UpdateChecked { force, result } => match result {
                Ok(Some(release))
                    if crate::update::compare(&self.state.version, &release.version).is_lt() =>
                {
                    let silenced = matches!(&self.state.update, UpdateState::Available { release: current, silenced: true } if current.version == release.version);
                    self.state.update = UpdateState::Available { release, silenced };
                }
                Ok(Some(_)) => {
                    if force {
                        self.feedback(
                            format!("已是最新版本 {}", self.state.version),
                            FeedbackKind::Success,
                            None,
                            false,
                        );
                    }
                }
                Ok(None) => (),
                Err(e) => {
                    if force {
                        self.feedback(
                            format!("检查更新失败：{e}"),
                            FeedbackKind::Warning,
                            None,
                            false,
                        );
                    }
                }
            },
            Message::UpdateProgress { received, total } => {
                if let UpdateState::Downloading {
                    percent,
                    received: current,
                    total: size,
                    ..
                } = &mut self.state.update
                {
                    *current = received;
                    *size = total;
                    *percent = total
                        .filter(|n| *n > 0)
                        .map_or(0, |n| ((received.saturating_mul(100) / n).min(100)) as u8);
                }
            }
            Message::UpdateDownloaded { release, result } => match result {
                Ok(prepared) => self.state.update = UpdateState::Ready { release, prepared },
                Err(e) => {
                    self.state.update = UpdateState::Available {
                        release,
                        silenced: false,
                    };
                    self.feedback(
                        format!("下载更新失败：{e}"),
                        FeedbackKind::Warning,
                        None,
                        false,
                    );
                }
            },
            Message::UpdateInstalled(result) => {
                if let Err(e) = result {
                    self.feedback(
                        format!("安装更新失败：{e}"),
                        FeedbackKind::Warning,
                        None,
                        false,
                    );
                }
            }
        }
    }
}

async fn switch_account(
    target: &dyn TargetAdapter,
    credentials: &dyn CredentialStore,
    accounts: &[AccountRecord],
    record: &AccountRecord,
    new_login: bool,
) -> SwitchResult {
    async fn run(
        target: &dyn TargetAdapter,
        credentials: &dyn CredentialStore,
        accounts: &[AccountRecord],
        record: &AccountRecord,
        new_login: bool,
    ) -> Result<(Option<String>, bool), String> {
        let snapshot = load_snapshot(credentials, record.id).map_err(SnapshotLoadError::message)?;
        if !emails_match(&snapshot.email, &record.email) {
            return Err(format!(
                "切换后身份校验失败（期望 {}，实际 {}）",
                record.email, snapshot.email
            ));
        }
        let mut previous = target.capture_snapshot().ok();
        if let Some(live) = &previous {
            if let Some(holder) = accounts
                .iter()
                .find(|a| emails_match(&a.email, &live.email))
            {
                save_snapshot(credentials, holder.id, live)?;
            }
        }
        let running = target.is_running();
        let relaunch = running || new_login;
        if relaunch && !target.can_launch() {
            return Err("找不到 Pen，请先安装后再切换".into());
        }
        if running && !target.terminate_and_wait(Duration::from_secs(15)).await {
            return Err("目标软件未能退出，可能正在等待保存确认。请先退出 Pen 后再切换".into());
        }
        let applied = (|| -> Result<(), String> {
            if let Ok(final_snapshot) = target.capture_snapshot() {
                previous = Some(final_snapshot.clone());
                if let Some(holder) = accounts
                    .iter()
                    .find(|a| emails_match(&a.email, &final_snapshot.email))
                {
                    save_snapshot(credentials, holder.id, &final_snapshot)?;
                }
            }
            target.apply_snapshot(&snapshot)?;
            if new_login {
                save_snapshot(credentials, record.id, &snapshot)?;
            }
            Ok(())
        })();
        if let Err(original) = applied {
            if let Some(previous) = previous {
                if target.apply_snapshot(&previous).is_err() {
                    return Err("切换失败，原登录状态恢复失败，请在 Pen 中重新登录".into());
                }
            }
            if running {
                if let Err(launch) = target.launch().await {
                    return Err(format!("{original}（恢复启动也失败：{launch}）"));
                }
            }
            return Err(original);
        }
        let launch_error = if relaunch {
            target.launch().await.err()
        } else {
            None
        };
        Ok((launch_error, relaunch))
    }
    match run(target, credentials, accounts, record, new_login).await {
        Ok((launch_error, relaunched)) => SwitchResult::Completed {
            launch_error,
            relaunched,
        },
        Err(error) => SwitchResult::Failed(error),
    }
}
