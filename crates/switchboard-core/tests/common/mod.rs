#![allow(dead_code)]

use std::{
    collections::HashMap,
    future::Future,
    io,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::Duration,
};
use switchboard_core::{
    app_model::AppServices,
    quota::QuotaResult,
    store::{CredentialStore, DataDirectory},
    target::{TargetAdapter, TargetFuture, TargetSnapshot},
    update::{AppRelease, PlatformAsset},
};
use tokio::sync::{mpsc, Notify};

pub struct TempDataDir(pub tempfile::TempDir);

impl TempDataDir {
    pub fn new() -> Self {
        Self(tempfile::tempdir().expect("create temp dir"))
    }
    pub fn path(&self) -> &Path {
        self.0.path()
    }
}

impl DataDirectory for TempDataDir {
    fn data_dir(&self) -> PathBuf {
        self.0.path().to_path_buf()
    }
}

pub fn make_snapshot(email: &str) -> TargetSnapshot {
    TargetSnapshot {
        session_file: format!(r#"{{"email":"{email}","token":"test-token"}}"#),
        email: email.to_string(),
        token: Some("test-token".to_string()),
    }
}

pub fn make_snapshot_with_token(email: &str, token: &str) -> TargetSnapshot {
    TargetSnapshot {
        session_file: format!(r#"{{"email":"{email}","token":"{token}"}}"#),
        email: email.to_string(),
        token: Some(token.to_string()),
    }
}

#[derive(Default)]
pub struct MemoryCredentialStore {
    pub items: Mutex<HashMap<String, Vec<u8>>>,
    pub fail_delete: AtomicBool,
    pub fail_save: AtomicBool,
}

impl MemoryCredentialStore {
    pub fn insert_snapshot(&self, id: uuid::Uuid, snapshot: &TargetSnapshot) {
        let data = serde_json::to_vec(snapshot).unwrap();
        self.items
            .lock()
            .unwrap()
            .insert(id.to_string().to_uppercase(), data);
    }
    pub fn load_snapshot(&self, id: uuid::Uuid) -> Option<TargetSnapshot> {
        let key = id.to_string().to_uppercase();
        let data = self.items.lock().unwrap().get(&key).cloned()?;
        serde_json::from_slice(&data).ok()
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn save(&self, account: &str, data: &[u8]) -> io::Result<()> {
        if self.fail_save.load(Ordering::SeqCst) {
            return Err(io::Error::other("simulated credential save error"));
        }
        self.items
            .lock()
            .unwrap()
            .insert(account.to_uppercase(), data.to_vec());
        Ok(())
    }

    fn load(&self, account: &str) -> io::Result<Vec<u8>> {
        self.items
            .lock()
            .unwrap()
            .get(&account.to_uppercase())
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "本地没有该账号的登录凭据"))
    }

    fn delete(&self, account: &str) -> io::Result<()> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(io::Error::other("simulated credential delete error"));
        }
        self.items.lock().unwrap().remove(&account.to_uppercase());
        Ok(())
    }
}

pub struct FakeTarget {
    pub session: Mutex<Option<TargetSnapshot>>,
    pub renewed_on_quit: Mutex<Option<TargetSnapshot>>,
    pub running: AtomicBool,
    pub quit_count: AtomicUsize,
    pub launch_count: AtomicUsize,
    pub fail_email: Mutex<Option<String>>,
    pub refuse_quit: AtomicBool,
    pub can_launch_flag: AtomicBool,
    pub launch_error: Mutex<Option<String>>,
    pub rollback_fail_email: Mutex<Option<String>>,
    pub quit_delay: Mutex<Duration>,
}

impl Default for FakeTarget {
    fn default() -> Self {
        Self {
            session: Mutex::new(Some(make_snapshot("a@example.test"))),
            renewed_on_quit: Mutex::new(None),
            running: AtomicBool::new(false),
            quit_count: AtomicUsize::new(0),
            launch_count: AtomicUsize::new(0),
            fail_email: Mutex::new(None),
            refuse_quit: AtomicBool::new(false),
            can_launch_flag: AtomicBool::new(true),
            launch_error: Mutex::new(None),
            rollback_fail_email: Mutex::new(None),
            quit_delay: Mutex::new(Duration::ZERO),
        }
    }
}

impl FakeTarget {
    pub fn set_session(&self, s: Option<TargetSnapshot>) {
        *self.session.lock().unwrap() = s;
    }
    pub fn get_session(&self) -> Option<TargetSnapshot> {
        self.session.lock().unwrap().clone()
    }
}

impl TargetAdapter for FakeTarget {
    fn live_identity(&self) -> Option<String> {
        self.session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.email.clone())
    }

    fn capture_snapshot(&self) -> Result<TargetSnapshot, String> {
        self.session
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| "目标软件当前没有登录账号".to_string())
    }

    fn apply_snapshot(&self, snapshot: &TargetSnapshot) -> Result<(), String> {
        if self
            .rollback_fail_email
            .lock()
            .unwrap()
            .as_deref()
            .is_some_and(|e| e.eq_ignore_ascii_case(&snapshot.email))
        {
            return Err("simulated rollback failure".into());
        }
        *self.session.lock().unwrap() = Some(snapshot.clone());
        if self
            .fail_email
            .lock()
            .unwrap()
            .as_deref()
            .is_some_and(|e| e.eq_ignore_ascii_case(&snapshot.email))
        {
            return Err(format!(
                "切换后身份校验失败（期望 {}，实际 无）",
                snapshot.email
            ));
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn terminate_and_wait(&self, _timeout: Duration) -> TargetFuture<'_, bool> {
        Box::pin(async move {
            let delay = *self.quit_delay.lock().unwrap();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            self.quit_count.fetch_add(1, Ordering::SeqCst);
            if self.refuse_quit.load(Ordering::SeqCst) {
                return false;
            }
            self.running.store(false, Ordering::SeqCst);
            if let Some(renewed) = self.renewed_on_quit.lock().unwrap().take() {
                *self.session.lock().unwrap() = Some(renewed);
            }
            true
        })
    }

    fn can_launch(&self) -> bool {
        self.can_launch_flag.load(Ordering::SeqCst)
    }

    fn launch(&self) -> TargetFuture<'_, Result<(), String>> {
        Box::pin(async move {
            self.launch_count.fetch_add(1, Ordering::SeqCst);
            if let Some(err) = self.launch_error.lock().unwrap().clone() {
                return Err(err);
            }
            self.running.store(true, Ordering::SeqCst);
            Ok(())
        })
    }
}

pub type QuotaFetchHook = Box<dyn Fn(&TargetSnapshot) + Send + Sync>;

pub struct FakeServices {
    // Login
    pub login_result: Mutex<Result<TargetSnapshot, String>>,
    pub login_delay: Mutex<Duration>,
    pub login_calls: AtomicUsize,
    pub login_cancel_calls: AtomicUsize,
    pub login_notify: Notify,

    // Quota
    pub quota_result: Mutex<QuotaResult>,
    pub quota_delay: Mutex<Duration>,
    pub quota_fetched: Mutex<Vec<String>>,
    pub quota_active: AtomicUsize,
    pub quota_max_active: AtomicUsize,
    pub on_quota_fetch: Mutex<Option<QuotaFetchHook>>,

    // Launch at login
    pub launch_at_login_state: AtomicBool,
    pub set_launch_error: Mutex<Option<String>>,

    // Updates
    pub update_repo: Mutex<String>,
    pub update_public_key: Mutex<Option<[u8; 32]>>,
    pub update_platform: PlatformAsset,
    pub feed_calls: AtomicUsize,
    pub feed_result: Mutex<Result<Option<AppRelease>, String>>,
    pub download_calls: AtomicUsize,
    pub download_chunks: Mutex<Vec<Vec<u8>>>,
    pub download_total: Mutex<Option<u64>>,
    pub download_delay: Mutex<Duration>,
    pub download_error: Mutex<Option<String>>,
    pub signature_value: Mutex<Result<String, String>>,
    pub prepare_calls: AtomicUsize,
    pub prepare_result: Mutex<Result<String, String>>,
    pub install_calls: AtomicUsize,
    pub install_result: Mutex<Result<(), String>>,
    pub opened_pages: Mutex<Vec<String>>,
}

impl Default for FakeServices {
    fn default() -> Self {
        Self {
            login_result: Mutex::new(Ok(make_snapshot("new@example.test"))),
            login_delay: Mutex::new(Duration::ZERO),
            login_calls: AtomicUsize::new(0),
            login_cancel_calls: AtomicUsize::new(0),
            login_notify: Notify::new(),
            quota_result: Mutex::new(QuotaResult::Quota {
                used: 2,
                total: 5,
                plan: None,
            }),
            quota_delay: Mutex::new(Duration::ZERO),
            quota_fetched: Mutex::new(Vec::new()),
            quota_active: AtomicUsize::new(0),
            quota_max_active: AtomicUsize::new(0),
            on_quota_fetch: Mutex::new(None),
            launch_at_login_state: AtomicBool::new(false),
            set_launch_error: Mutex::new(None),
            update_repo: Mutex::new("gaogep/pencil-account-switcher".into()),
            update_public_key: Mutex::new(None),
            update_platform: PlatformAsset::Mac,
            feed_calls: AtomicUsize::new(0),
            feed_result: Mutex::new(Ok(None)),
            download_calls: AtomicUsize::new(0),
            download_chunks: Mutex::new(vec![b"zip-bytes".to_vec()]),
            download_total: Mutex::new(Some(9)),
            download_delay: Mutex::new(Duration::ZERO),
            download_error: Mutex::new(None),
            signature_value: Mutex::new(Ok("invalid-sig".into())),
            prepare_calls: AtomicUsize::new(0),
            prepare_result: Mutex::new(Ok("/tmp/prepared.app".into())),
            install_calls: AtomicUsize::new(0),
            install_result: Mutex::new(Ok(())),
            opened_pages: Mutex::new(Vec::new()),
        }
    }
}

impl AppServices for FakeServices {
    fn login(&self) -> TargetFuture<'_, Result<TargetSnapshot, String>> {
        Box::pin(async move {
            self.login_calls.fetch_add(1, Ordering::SeqCst);
            let delay = *self.login_delay.lock().unwrap();
            if !delay.is_zero() {
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = self.login_notify.notified() => {
                        return Err("网页登录已取消或超时，原账号保持不变".into());
                    }
                }
            }
            self.login_result.lock().unwrap().clone()
        })
    }

    fn cancel_login(&self) {
        self.login_cancel_calls.fetch_add(1, Ordering::SeqCst);
        self.login_notify.notify_waiters();
    }

    fn quota<'a>(&'a self, snapshot: &'a TargetSnapshot) -> TargetFuture<'a, QuotaResult> {
        Box::pin(async move {
            let cur = self.quota_active.fetch_add(1, Ordering::SeqCst) + 1;
            self.quota_max_active.fetch_max(cur, Ordering::SeqCst);
            self.quota_fetched
                .lock()
                .unwrap()
                .push(snapshot.email.clone());
            if let Some(cb) = self.on_quota_fetch.lock().unwrap().as_ref() {
                cb(snapshot);
            }
            let delay = *self.quota_delay.lock().unwrap();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            self.quota_active.fetch_sub(1, Ordering::SeqCst);
            let res = self.quota_result.lock().unwrap();
            match &*res {
                QuotaResult::Quota { used, total, plan } => QuotaResult::Quota {
                    used: *used,
                    total: *total,
                    plan: plan.clone(),
                },
                QuotaResult::Unsupported => QuotaResult::Unsupported,
                QuotaResult::Expired => QuotaResult::Expired,
                QuotaResult::Failure(msg) => QuotaResult::Failure(msg.clone()),
            }
        })
    }

    fn launch_at_login(&self) -> Result<bool, String> {
        Ok(self.launch_at_login_state.load(Ordering::SeqCst))
    }

    fn set_launch_at_login(&self, enabled: bool) -> Result<(), String> {
        if let Some(err) = self.set_launch_error.lock().unwrap().clone() {
            return Err(err);
        }
        self.launch_at_login_state.store(enabled, Ordering::SeqCst);
        Ok(())
    }

    fn update_repo(&self) -> &str {
        // Leak-free way: return static str or match; update_repo is checked for emptiness
        // We use a leaked or fixed check; wait trait returns `&str` tied to `&self`.
        // Since MutexGuard can't return `&str`, use AtomicBool or Box::leak for tests.
        if self.update_repo.lock().unwrap().is_empty() {
            ""
        } else {
            "gaogep/pencil-account-switcher"
        }
    }

    fn update_public_key(&self) -> Option<[u8; 32]> {
        *self.update_public_key.lock().unwrap()
    }

    fn update_platform(&self) -> PlatformAsset {
        self.update_platform
    }

    fn fetch_release(&self) -> TargetFuture<'_, Result<Option<AppRelease>, String>> {
        Box::pin(async move {
            self.feed_calls.fetch_add(1, Ordering::SeqCst);
            self.feed_result.lock().unwrap().clone()
        })
    }

    fn download_update<'a>(
        &'a self,
        _url: &'a str,
        progress: mpsc::UnboundedSender<(u64, Option<u64>)>,
    ) -> TargetFuture<'a, Result<Vec<u8>, String>> {
        Box::pin(async move {
            self.download_calls.fetch_add(1, Ordering::SeqCst);
            let delay = *self.download_delay.lock().unwrap();
            let err = self.download_error.lock().unwrap().clone();
            if let Some(err) = err {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                return Err(err);
            }
            let total = *self.download_total.lock().unwrap();
            let chunks = self.download_chunks.lock().unwrap().clone();
            let mut buf = Vec::new();
            for chunk in chunks {
                buf.extend_from_slice(&chunk);
                let _ = progress.send((buf.len() as u64, total));
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
            Ok(buf)
        })
    }

    fn download_signature<'a>(&'a self, _url: &'a str) -> TargetFuture<'a, Result<String, String>> {
        Box::pin(async move { self.signature_value.lock().unwrap().clone() })
    }

    fn prepare_update<'a>(
        &'a self,
        _package: &'a Path,
        _version: &'a str,
    ) -> TargetFuture<'a, Result<String, String>> {
        Box::pin(async move {
            self.prepare_calls.fetch_add(1, Ordering::SeqCst);
            self.prepare_result.lock().unwrap().clone()
        })
    }

    fn install_update<'a>(&'a self, _prepared: &'a str) -> TargetFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.install_calls.fetch_add(1, Ordering::SeqCst);
            self.install_result.lock().unwrap().clone()
        })
    }

    fn open_update_page(&self, url: &str) -> Result<(), String> {
        self.opened_pages.lock().unwrap().push(url.to_string());
        Ok(())
    }
}

pub struct FakeAuthWindow {
    pub opened_urls: Mutex<Vec<String>>,
    pub open_notify: Notify,
    pub cancel_count: AtomicUsize,
    pub open_error: Mutex<Option<String>>,
    pub close_notify: Notify,
    pub close_immediately: AtomicBool,
    pub close_error: Mutex<Option<String>>,
}

impl Default for FakeAuthWindow {
    fn default() -> Self {
        Self {
            opened_urls: Mutex::new(Vec::new()),
            open_notify: Notify::new(),
            cancel_count: AtomicUsize::new(0),
            open_error: Mutex::new(None),
            close_notify: Notify::new(),
            close_immediately: AtomicBool::new(false),
            close_error: Mutex::new(None),
        }
    }
}

impl switchboard_core::web_login::AuthWindow for FakeAuthWindow {
    fn open(&self, url: &str) -> Result<(), String> {
        if let Some(err) = self.open_error.lock().unwrap().clone() {
            return Err(err);
        }
        self.opened_urls.lock().unwrap().push(url.to_string());
        self.open_notify.notify_waiters();
        Ok(())
    }

    fn closed(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            if !self.close_immediately.load(Ordering::SeqCst) {
                self.close_notify.notified().await;
            }
            if let Some(err) = self.close_error.lock().unwrap().clone() {
                return Err(err);
            }
            Ok(())
        })
    }

    fn cancel(&self) {
        self.cancel_count.fetch_add(1, Ordering::SeqCst);
    }
}

pub struct SharedAuthWindow(pub std::sync::Arc<FakeAuthWindow>);

impl switchboard_core::web_login::AuthWindow for SharedAuthWindow {
    fn open(&self, url: &str) -> Result<(), String> {
        self.0.open(url)
    }

    fn closed(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        self.0.closed()
    }

    fn cancel(&self) {
        self.0.cancel()
    }
}
