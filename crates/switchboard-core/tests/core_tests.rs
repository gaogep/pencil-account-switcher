mod common;

use base64::{engine::general_purpose::STANDARD, Engine};
use common::{
    make_snapshot, make_snapshot_with_token, FakeAuthWindow, FakeServices, FakeTarget,
    MemoryCredentialStore, SharedAuthWindow, TempDataDir,
};
use ed25519_dalek::{Signer, SigningKey};
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use switchboard_core::{
    app_model::{self, AppHandle, Command, FeedbackKind, UpdateState, ViewState},
    model::{AccountRecord, AuthState, StoreFile},
    pen_adapter::{PenPlatform, PenTargetAdapter},
    quota::{parse_body, status_result, DeviceIdentity, PenQuotaProvider, QuotaResult},
    snapshot,
    store::{atomic_write, CredentialStore, FileCredentialStore, LoadStore, Store},
    target::{emails_match, TargetAdapter, TargetFuture},
    update::{self, AppRelease, PlatformAsset, UpdateSchedule},
    web_login::{callback_code, WebLogin},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

async fn wait_until<F>(handle: &AppHandle, mut pred: F) -> ViewState
where
    F: FnMut(&ViewState) -> bool,
{
    let mut rx = handle.subscribe();
    for _ in 0..200 {
        let state = rx.borrow().clone();
        if pred(&state) {
            return state;
        }
        if tokio::time::timeout(Duration::from_millis(50), rx.changed())
            .await
            .is_err()
        {
            let state = handle.state();
            if pred(&state) {
                return state;
            }
        }
    }
    panic!(
        "timed out waiting for expected ViewState: {:?}",
        handle.state()
    );
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1.0.1")
}

fn seed_store(dir: &TempDataDir, accounts: Vec<AccountRecord>) -> Store {
    let store = Store::new(dir);
    store
        .write(&StoreFile {
            store_version: 1,
            accounts,
        })
        .expect("write initial store");
    store
}

fn sample_account(email: &str, is_current: bool) -> AccountRecord {
    AccountRecord {
        id: Uuid::new_v4(),
        email: email.into(),
        plan: None,
        is_current,
        used_days: None,
        total_days: None,
        last_quota_update: None,
        last_error: None,
        auth_state: AuthState::Ok,
    }
}

struct DummyPenPlatform {
    running: bool,
}
impl PenPlatform for DummyPenPlatform {
    fn is_running(&self) -> bool {
        self.running
    }
    fn terminate_and_wait(&self, _timeout: Duration) -> TargetFuture<'_, bool> {
        Box::pin(async { true })
    }
    fn can_launch(&self) -> bool {
        true
    }
    fn launch(&self) -> TargetFuture<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
}

struct StaticIdentity {
    host: String,
    os: &'static str,
    arch: &'static str,
}
impl DeviceIdentity for StaticIdentity {
    fn hostname(&self) -> String {
        self.host.clone()
    }
    fn os_tag(&self) -> &'static str {
        self.os
    }
    fn architecture(&self) -> &'static str {
        self.arch
    }
}

// ---------------------------------------------------------------------------
// 身份同步: C-ID-01, C-ID-02, C-ID-03
// ---------------------------------------------------------------------------

#[tokio::test]
async fn c_id_01_02_03_identity_sync_and_case_insensitive_match() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let store = seed_store(&dir, vec![a.clone(), b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.set_session(Some(make_snapshot("A@EXAMPLE.TEST")));
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());

    let handle = app_model::start(
        target.clone(),
        services,
        creds,
        store,
        "1.0.1".into(),
        false,
    );

    // C-ID-03: case-insensitive match marks a@example.test as current
    let s = wait_until(&handle, |s| s.live_email.is_some()).await;
    assert_eq!(
        s.accounts.iter().find(|x| x.is_current).map(|x| x.id),
        Some(a.id)
    );
    assert_eq!(s.unregistered_live_email, None);
    assert!(emails_match("a@example.test", "A@Example.Test"));

    // C-ID-01: logout (session disappears) clears current account
    target.set_session(None);
    handle.send(Command::SyncIdentity);
    let s = wait_until(&handle, |s| s.live_email.is_none()).await;
    assert!(s.accounts.iter().all(|x| !x.is_current));
    assert_eq!(s.unregistered_live_email, None);

    // C-ID-02: unregistered email clears current account and sets unregistered_live_email
    target.set_session(Some(make_snapshot("other@example.test")));
    handle.send(Command::SyncIdentity);
    let s = wait_until(&handle, |s| {
        s.unregistered_live_email.as_deref() == Some("other@example.test")
    })
    .await;
    assert!(s.accounts.iter().all(|x| !x.is_current));
}

// ---------------------------------------------------------------------------
// 切换: C-SW-01 ~ C-SW-12
// ---------------------------------------------------------------------------

#[tokio::test]
async fn c_sw_01_missing_credentials_do_not_quit_pen() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let store = seed_store(&dir, vec![a, b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());

    let handle = app_model::start(
        target.clone(),
        services,
        creds,
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    handle.send(Command::SwitchTo(b.id));
    let s = wait_until(&handle, |s| {
        s.switching_to.is_none()
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("凭据缺失"))
    })
    .await;
    assert_eq!(target.quit_count.load(Ordering::SeqCst), 0);
    assert!(target.is_running());
    assert_eq!(s.feedback.unwrap().kind as u8, FeedbackKind::Warning as u8);
}

#[tokio::test]
async fn c_sw_02_03_04_failed_write_rolls_back_restarts_pen_and_keeps_identity() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let store = seed_store(&dir, vec![a.clone(), b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    *target.fail_email.lock().unwrap() = Some(b.email.clone());
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());
    creds.insert_snapshot(b.id, &make_snapshot(&b.email));

    let handle = app_model::start(
        target.clone(),
        services,
        creds,
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    handle.send(Command::SwitchTo(b.id));
    let s = wait_until(&handle, |s| {
        s.switching_to.is_none()
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("切换失败"))
    })
    .await;
    assert_eq!(target.get_session().unwrap().email, a.email);
    assert!(target.is_running());
    assert_eq!(target.launch_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.accounts.iter().find(|x| x.is_current).map(|x| x.id),
        Some(a.id)
    );
}

#[test]
fn c_sw_05_06_pen_adapter_restore_missing_and_reject_mismatched_snapshot() {
    let dir = TempDataDir::new();
    let adapter = PenTargetAdapter::new(
        dir.path().to_path_buf(),
        DummyPenPlatform { running: false },
    );

    // C-SW-05: restore creates missing ~/.pencil/session-desktop.json
    assert!(adapter.live_identity().is_none());
    adapter
        .apply_snapshot(&make_snapshot("a@example.test"))
        .expect("restore creates missing session file");
    assert_eq!(adapter.live_identity().as_deref(), Some("a@example.test"));

    // C-SW-06: mismatched email in session_file is rejected before write
    let mut mismatched = make_snapshot("b@example.test");
    mismatched.session_file = r#"{"email":"other@example.test","token":"test-token"}"#.into();
    let err = adapter.apply_snapshot(&mismatched).unwrap_err();
    assert!(err.contains("期望 b@example.test"));
    assert_eq!(adapter.live_identity().as_deref(), Some("a@example.test"));

    // Fixture check (TEST_PLAN 3.3): extra fields like lastOnlineAt & currentWorkspace are preserved
    let fixture_session = fs::read_to_string(fixture_dir().join("session-desktop.json")).unwrap();
    let snap = switchboard_core::target::TargetSnapshot {
        session_file: fixture_session.clone(),
        email: "fixture.one@example.test".into(),
        token: Some("fake-token-one".into()),
    };
    adapter.apply_snapshot(&snap).unwrap();
    let captured = adapter.capture_snapshot().unwrap();
    assert_eq!(captured.session_file, fixture_session);
}

#[tokio::test]
async fn c_sw_07_renewed_token_on_quit_is_saved_and_used_for_rollback() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let store = seed_store(&dir, vec![a.clone(), b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    *target.renewed_on_quit.lock().unwrap() =
        Some(make_snapshot_with_token(&a.email, "renewed-token-on-quit"));
    *target.fail_email.lock().unwrap() = Some(b.email.clone());

    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());
    creds.insert_snapshot(a.id, &make_snapshot_with_token(&a.email, "old-token"));
    creds.insert_snapshot(b.id, &make_snapshot(&b.email));

    let handle = app_model::start(
        target.clone(),
        services,
        creds.clone(),
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    handle.send(Command::SwitchTo(b.id));
    wait_until(&handle, |s| {
        s.switching_to.is_none()
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("切换失败"))
    })
    .await;

    // Rolled-back live session and saved credential both have the renewed token
    assert_eq!(
        target.get_session().unwrap().token.as_deref(),
        Some("renewed-token-on-quit")
    );
    assert_eq!(
        creds.load_snapshot(a.id).unwrap().token.as_deref(),
        Some("renewed-token-on-quit")
    );
}

#[tokio::test]
async fn c_sw_08_quit_timeout_refuses_switch_and_preserves_session() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let store = seed_store(&dir, vec![a.clone(), b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    target.refuse_quit.store(true, Ordering::SeqCst);
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());
    creds.insert_snapshot(b.id, &make_snapshot(&b.email));

    let handle = app_model::start(
        target.clone(),
        services,
        creds,
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    handle.send(Command::SwitchTo(b.id));
    let s = wait_until(&handle, |s| {
        s.switching_to.is_none()
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("未能退出"))
    })
    .await;
    assert!(target.is_running());
    assert_eq!(target.get_session().unwrap().email, a.email);
    assert!(s.feedback.unwrap().text.contains("请先退出 Pen"));
}

#[tokio::test]
async fn c_sw_09_launch_failure_still_completes_switch_and_skips_soft_verify() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let store = seed_store(&dir, vec![a, b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    *target.launch_error.lock().unwrap() = Some("找不到 Pen，请先安装后再切换".into());
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());
    creds.insert_snapshot(b.id, &make_snapshot(&b.email));

    let handle = app_model::start(
        target.clone(),
        services,
        creds,
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    handle.send(Command::SwitchTo(b.id));
    let s = wait_until(&handle, |s| {
        s.switching_to.is_none()
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("请手动打开"))
    })
    .await;
    assert_eq!(
        s.accounts.iter().find(|x| x.is_current).map(|x| x.id),
        Some(b.id)
    );
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));
}

#[tokio::test]
async fn c_sw_10_soft_verify_warns_without_rollback() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let store = seed_store(&dir, vec![a, b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());
    creds.insert_snapshot(b.id, &make_snapshot(&b.email));

    let handle = app_model::start(
        target.clone(),
        services,
        creds,
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    handle.send(Command::SwitchTo(b.id));
    let s = wait_until(&handle, |s| {
        s.switching_to.is_none() && s.accounts.iter().any(|x| x.id == b.id && x.is_current)
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Success
    ));

    // Hijack live session while 4s soft verify timer is pending
    target.set_session(Some(make_snapshot("hijacked@example.test")));
    tokio::time::sleep(Duration::from_millis(4200)).await;
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text.contains("登录身份已变化"))
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));
    assert_eq!(target.get_session().unwrap().email, "hijacked@example.test");
}

#[tokio::test]
async fn c_sw_11_12_refresh_blocks_switch_and_concurrent_switch_runs_once() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let b = sample_account("b@example.test", false);
    let c = sample_account("c@example.test", false);
    let store = seed_store(&dir, vec![a, b.clone(), c.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    let services = Arc::new(FakeServices::default());
    *services.quota_delay.lock().unwrap() = Duration::from_millis(250);
    let creds = Arc::new(MemoryCredentialStore::default());
    creds.insert_snapshot(b.id, &make_snapshot(&b.email));
    creds.insert_snapshot(c.id, &make_snapshot(&c.email));

    let handle = app_model::start(
        target.clone(),
        services.clone(),
        creds,
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    // C-SW-11: switch during refresh is rejected
    handle.send(Command::RefreshQuotas);
    wait_until(&handle, |s| s.refreshing).await;
    handle.send(Command::SwitchTo(b.id));
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text == "正在刷新额度，请稍后再切换")
    })
    .await;
    assert_eq!(target.quit_count.load(Ordering::SeqCst), 0);
    assert!(s.feedback.unwrap().text.contains("请稍后再切换"));
    wait_until(&handle, |s| !s.refreshing).await;

    // C-SW-12: two concurrent switches only execute one
    handle.send(Command::SwitchTo(b.id));
    handle.send(Command::SwitchTo(c.id));
    let s = wait_until(&handle, |s| {
        s.switching_to.is_none() && s.accounts.iter().any(|x| x.id == b.id && x.is_current)
    })
    .await;
    assert_eq!(target.quit_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.accounts.iter().find(|x| x.is_current).map(|x| x.id),
        Some(b.id)
    );
}

// ---------------------------------------------------------------------------
// 添加 / 导入 / 移除: C-AC-01 ~ C-AC-07
// ---------------------------------------------------------------------------

#[tokio::test]
async fn c_ac_01_02_03_web_login_add_saved_but_failed_and_cancel() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let store = seed_store(&dir, vec![a]);

    let target = Arc::new(FakeTarget::default());
    target.running.store(true, Ordering::SeqCst);
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());

    let handle = app_model::start(
        target.clone(),
        services.clone(),
        creds.clone(),
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    // C-AC-01: web login completes, saves, switches, refreshes (adding_account stays true while switching)
    *target.quit_delay.lock().unwrap() = Duration::from_millis(60);
    handle.send(Command::AddAccount);
    let switching_state = wait_until(&handle, |s| s.switching_to.is_some()).await;
    assert!(switching_state.adding_account);
    *target.quit_delay.lock().unwrap() = Duration::ZERO;
    let s = wait_until(&handle, |s| {
        !s.adding_account && !s.refreshing && s.accounts.len() == 2
    })
    .await;
    assert_eq!(
        s.accounts
            .iter()
            .find(|x| x.is_current)
            .map(|x| x.email.as_str()),
        Some("new@example.test")
    );
    assert_eq!(target.quit_count.load(Ordering::SeqCst), 1);
    assert_eq!(target.launch_count.load(Ordering::SeqCst), 1);

    // Re-login same email deduplicates and updates token
    let logins_before = services.login_calls.load(Ordering::SeqCst);
    *services.login_result.lock().unwrap() = Ok(make_snapshot_with_token(
        "new@example.test",
        "renewed-token",
    ));
    handle.send(Command::AddAccount);
    let s = wait_until(&handle, |s| {
        services.login_calls.load(Ordering::SeqCst) > logins_before
            && !s.adding_account
            && !s.refreshing
            && s.feedback.as_ref().is_some_and(|f| f.text == "额度已刷新")
    })
    .await;
    assert_eq!(s.accounts.len(), 2);
    let new_id = s
        .accounts
        .iter()
        .find(|x| x.email == "new@example.test")
        .unwrap()
        .id;
    assert_eq!(
        creds.load_snapshot(new_id).unwrap().token.as_deref(),
        Some("renewed-token")
    );

    // C-AC-02: saved-but-failed stays a warning
    *services.login_result.lock().unwrap() = Ok(make_snapshot("fail-switch@example.test"));
    *target.fail_email.lock().unwrap() = Some("fail-switch@example.test".into());
    handle.send(Command::AddAccount);
    let s = wait_until(&handle, |s| {
        !s.adding_account
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("账号已保存，但切换失败"))
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));
    *target.fail_email.lock().unwrap() = None;

    // C-AC-03: cancel web login
    let quit_before = target.quit_count.load(Ordering::SeqCst);
    *services.login_delay.lock().unwrap() = Duration::from_secs(10);
    handle.send(Command::AddAccount);
    wait_until(&handle, |s| s.adding_account).await;
    handle.send(Command::CancelAddAccount);
    let s = wait_until(&handle, |s| {
        !s.adding_account
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text == "网页登录已取消或超时，原账号保持不变")
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));
    assert_eq!(target.quit_count.load(Ordering::SeqCst), quit_before);
}

#[tokio::test(start_paused = true)]
async fn c_ac_04_web_login_300s_timeout() {
    let dir = TempDataDir::new();
    let store = seed_store(&dir, vec![]);
    let target = Arc::new(FakeTarget::default());
    target.set_session(None);
    let services = Arc::new(FakeServices::default());
    *services.login_delay.lock().unwrap() = Duration::from_secs(600);
    let creds = Arc::new(MemoryCredentialStore::default());

    let handle = app_model::start(
        target,
        services.clone(),
        creds,
        store,
        "1.0.1".into(),
        false,
    );

    handle.send(Command::AddAccount);
    wait_until(&handle, |s| s.adding_account).await;

    tokio::time::advance(Duration::from_secs(301)).await;
    let s = wait_until(&handle, |s| !s.adding_account && s.feedback.is_some()).await;
    assert_eq!(services.login_cancel_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.feedback.as_ref().unwrap().text,
        "网页登录已取消或超时，原账号保持不变"
    );
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));
}

#[tokio::test]
async fn c_ac_05_06_07_import_existing_stale_remove_and_busy_remove() {
    let dir = TempDataDir::new();
    let a = sample_account("a@example.test", true);
    let mut b = sample_account("b@example.test", false);
    b.auth_state = AuthState::Expired;
    let store = seed_store(&dir, vec![a.clone(), b.clone()]);

    let target = Arc::new(FakeTarget::default());
    target.set_session(Some(make_snapshot_with_token(
        "b@example.test",
        "updated-tok",
    )));
    let services = Arc::new(FakeServices::default());
    *services.quota_delay.lock().unwrap() = Duration::from_millis(250);
    let creds = Arc::new(MemoryCredentialStore::default());
    creds.insert_snapshot(a.id, &make_snapshot(&a.email));

    let handle = app_model::start(
        target.clone(),
        services.clone(),
        creds.clone(),
        store,
        "1.0.1".into(),
        false,
    );
    wait_until(&handle, |s| s.live_email.is_some()).await;

    // C-AC-05 & I-009: import existing login preserves "登录状态已更新" before refresh runs
    handle.send(Command::ImportCurrentLogin);
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text.contains("登录状态已更新"))
    })
    .await;
    assert_eq!(s.accounts.len(), 2);
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Success
    ));
    assert_eq!(
        creds.load_snapshot(b.id).unwrap().token.as_deref(),
        Some("updated-tok")
    );
    // Wait for delayed quota refresh to complete
    wait_until(&handle, |s| {
        !s.refreshing && s.feedback.as_ref().is_some_and(|f| f.text == "额度已刷新")
    })
    .await;

    // C-AC-06: live current account (b) cannot be removed even if UI held stale state
    handle.send(Command::RemoveAccount(b.id));
    handle.flush().await;
    assert_eq!(handle.state().accounts.len(), 2);

    // C-AC-07: busy remove during refresh is rejected
    handle.send(Command::RefreshQuotas);
    wait_until(&handle, |s| s.refreshing).await;
    handle.send(Command::RemoveAccount(a.id));
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text.contains("有操作正在进行"))
    })
    .await;
    assert_eq!(s.accounts.len(), 2);
    wait_until(&handle, |s| !s.refreshing).await;

    // Removing non-current account succeeds when idle
    handle.send(Command::RemoveAccount(a.id));
    let s = wait_until(&handle, |s| {
        s.accounts.len() == 1
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("已移除"))
    })
    .await;
    assert!(creds.load(&a.id.to_string()).is_err());
    assert_eq!(s.accounts[0].id, b.id);
}

// ---------------------------------------------------------------------------
// 存储与损坏保护: C-ST-01 ~ C-ST-09
// ---------------------------------------------------------------------------

#[tokio::test]
async fn c_st_01_to_06_corrupt_and_newer_store_blocks_persistence_and_writes() {
    for (label, raw) in [
        ("corrupt", b"{".as_slice()),
        (
            "future_version",
            br#"{"storeVersion":2,"accounts":[]}"#.as_slice(),
        ),
    ] {
        let dir = TempDataDir::new();
        let store_path = dir.path().join("store.json");
        fs::write(&store_path, raw).unwrap();

        let store = Store::new(&dir);
        let target = Arc::new(FakeTarget::default());
        let services = Arc::new(FakeServices::default());
        let creds = Arc::new(MemoryCredentialStore::default());

        let handle = app_model::start(
            target,
            services.clone(),
            creds.clone(),
            store,
            "1.0.1".into(),
            false,
        );
        let s = wait_until(&handle, |s| s.persistence_blocked).await;
        assert!(
            s.persistence_blocked,
            "{label}: persistence should be blocked"
        );
        assert!(s.feedback.as_ref().unwrap().text.contains("账号列表已损坏"));

        // Backup file created and original file untouched
        let entries: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            entries
                .iter()
                .any(|name| name.starts_with("store.corrupt-") && name.ends_with(".json")),
            "{label}: corrupt backup missing"
        );
        assert_eq!(fs::read(&store_path).unwrap(), raw);

        // C-ST-02: locked import writes no credential file
        handle.send(Command::ImportCurrentLogin);
        let s = wait_until(&handle, |s| {
            s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("账号列表文件异常"))
        })
        .await;
        assert!(s.accounts.is_empty());
        assert!(creds.items.lock().unwrap().is_empty());

        // C-ST-03: locked add does not start web login or write credential
        handle.send(Command::AddAccount);
        handle.flush().await;
        assert_eq!(services.login_calls.load(Ordering::SeqCst), 0);
        assert!(!handle.state().adding_account);
        assert!(creds.items.lock().unwrap().is_empty());

        // C-ST-04: locked remove keeps account (even if somehow triggered)
        handle.send(Command::RemoveAccount(Uuid::new_v4()));
        handle.flush().await;
        assert_eq!(fs::read(&store_path).unwrap(), raw);
    }
}

#[tokio::test]
async fn c_st_07_08_09_coalesce_v101_fixture_roundtrip_and_unknown_auth() {
    // C-ST-09: AuthState decodes unknown string to AuthState::Unknown
    let json = r#"{"storeVersion":1,"accounts":[{"id":"11111111-1111-4111-8111-111111111111","email":"a@b.c","isCurrent":false,"authState":"brand_new_state"}]}"#;
    let parsed: StoreFile = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.accounts[0].auth_state, AuthState::Unknown);

    // C-ST-08: v1.0.1 fixture unmodified round-trip preserves exact bytes (I-010)
    let dir = TempDataDir::new();
    let fixture_store = fs::read(fixture_dir().join("store.json")).unwrap();
    fs::write(dir.path().join("store.json"), &fixture_store).unwrap();
    let store = Store::new(&dir);
    let LoadStore::Ready(loaded) = store.load() else {
        panic!("expected fixture store to load cleanly");
    };
    assert_eq!(loaded.store_version, 1);
    assert_eq!(loaded.accounts.len(), 2);
    assert_eq!(loaded.accounts[0].email, "fixture.one@example.test");
    assert_eq!(loaded.accounts[1].auth_state, AuthState::Expired);
    store.write(&loaded).unwrap();
    assert_eq!(
        fs::read(dir.path().join("store.json")).unwrap(),
        fixture_store
    );

    // I-010: modified re-encoding emits whole-second timestamps as integers (no trailing .0)
    let mut modified = loaded.clone();
    modified.accounts[0].last_quota_update = Some(816229440.0);
    let encoded_whole = serde_json::to_string(&modified).unwrap();
    assert!(encoded_whole.contains(r#""lastQuotaUpdate":816229440,"#));
    assert!(!encoded_whole.contains("816229440.0"));
    modified.accounts[0].last_quota_update = Some(816229440.5);
    let encoded_frac = serde_json::to_string(&modified).unwrap();
    assert!(encoded_frac.contains(r#""lastQuotaUpdate":816229440.5"#));

    // Load v1.0.1 credential fixture
    let cred_bytes =
        fs::read(fixture_dir().join("credentials/11111111-1111-4111-8111-111111111111.json"))
            .unwrap();
    let snap: switchboard_core::target::TargetSnapshot =
        serde_json::from_slice(&cred_bytes).unwrap();
    assert_eq!(snap.email, "fixture.one@example.test");
    assert_eq!(snap.token.as_deref(), Some("fake-token-one"));

    // C-ST-07: coalesced writes + flush on demand
    let dir2 = TempDataDir::new();
    let a = sample_account("a@example.test", false);
    let store2 = seed_store(&dir2, vec![a]);
    let target = Arc::new(FakeTarget::default());
    target.set_session(Some(make_snapshot("a@example.test")));
    let handle = app_model::start(
        target,
        Arc::new(FakeServices::default()),
        Arc::new(MemoryCredentialStore::default()),
        store2,
        "1.0.1".into(),
        false,
    );
    handle.send(Command::SyncIdentity);
    handle.send(Command::SyncIdentity);
    handle.flush().await;
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(dir2.path().join("store.json")).unwrap()).unwrap();
    assert!(raw.get("settings").is_none());
    assert_eq!(raw["accounts"][0]["isCurrent"], true);
}

// ---------------------------------------------------------------------------
// 凭据文件: C-CR-01 ~ C-CR-05
// ---------------------------------------------------------------------------

#[test]
fn c_cr_01_to_05_file_credentials_permissions_traversal_delete_and_session_mode() {
    let dir = TempDataDir::new();
    let store = FileCredentialStore::new(&dir);
    let id = Uuid::new_v4().to_string();
    let payload = b"credential-blob";

    // C-CR-01: round-trip
    store.save(&id, payload).unwrap();
    assert_eq!(store.load(&id).unwrap(), payload);

    // C-CR-02: macOS directory 0700 and file 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let cred_dir = dir.path().join("credentials");
        let dir_mode = fs::metadata(&cred_dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        let cred_file = cred_dir.join(format!("{}.json", id.to_uppercase()));
        let file_mode = fs::metadata(&cred_file).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);
    }

    // C-CR-03: non-UUID / path traversal rejected
    for bad in ["../evil", "not-a-uuid", "..\\evil", ""] {
        assert!(store.save(bad, payload).is_err(), "should reject {bad}");
        assert!(store.load(bad).is_err(), "load should reject {bad}");
    }

    // C-CR-04: delete makes missing, repeat delete is idempotent
    store.delete(&id).unwrap();
    assert_eq!(
        store.load(&id).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    store.delete(&id).unwrap();

    // C-CR-05: session file is 0600 after apply_snapshot
    let adapter = PenTargetAdapter::new(
        dir.path().to_path_buf(),
        DummyPenPlatform { running: false },
    );
    adapter
        .apply_snapshot(&make_snapshot("a@example.test"))
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let session = dir.path().join(".pencil/session-desktop.json");
        let mode = fs::metadata(&session).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

// ---------------------------------------------------------------------------
// 额度: C-QU-01 ~ C-QU-08
// ---------------------------------------------------------------------------

#[test]
fn c_qu_01_to_06_and_08_quota_parsing_status_and_device_id() {
    // C-QU-01: 401 -> Expired
    assert_eq!(status_result(401), Some(QuotaResult::Expired));
    // C-QU-02: 403 -> Failure without expiring
    assert!(
        matches!(status_result(403), Some(QuotaResult::Failure(msg)) if msg.contains("没有权限"))
    );
    assert_eq!(status_result(200), None);
    assert!(matches!(status_result(500), Some(QuotaResult::Failure(msg)) if msg.contains("500")));

    // C-QU-03: isMetered=false -> Unsupported
    assert_eq!(
        parse_body(br#"{"isMetered":false}"#),
        QuotaResult::Unsupported
    );
    // C-QU-04: missing used/limit -> Unsupported
    assert_eq!(parse_body(br#"{}"#), QuotaResult::Unsupported);
    assert_eq!(parse_body(br#"{"used":1}"#), QuotaResult::Unsupported);
    // C-QU-05: valid metered body -> Quota
    assert_eq!(
        parse_body(br#"{"isMetered":true,"used":1,"limit":5}"#),
        QuotaResult::Quota {
            used: 1,
            total: 5,
            plan: None
        }
    );
    // C-QU-06: invalid JSON -> Failure
    assert!(matches!(parse_body(b"not-json"), QuotaResult::Failure(_)));

    // C-QU-08: deviceId formula matches Pen desktop md5(hostname + os_tag + arch)
    let provider = PenQuotaProvider::new(
        "https://api.pencil.dev".into(),
        StaticIdentity {
            host: "test-host.local".into(),
            os: "darwin",
            arch: "arm64",
        },
    );
    let expected = format!("{:x}", md5::compute("test-host.localdarwinarm64"));
    assert_eq!(provider.device_id(), expected);
    assert_eq!(provider.device_id(), expected); // cached
}

#[tokio::test]
async fn c_qu_07_multi_account_refresh_concurrency_and_messages() {
    let dir = TempDataDir::new();
    let accounts: Vec<_> = (0..5)
        .map(|i| sample_account(&format!("user{i}@example.test"), i == 0))
        .collect();
    let store = seed_store(&dir, accounts.clone());

    let target = Arc::new(FakeTarget::default());
    target.set_session(None);
    let services = Arc::new(FakeServices::default());
    *services.quota_delay.lock().unwrap() = Duration::from_millis(80);
    let creds = Arc::new(MemoryCredentialStore::default());
    for acc in &accounts {
        creds.insert_snapshot(acc.id, &make_snapshot(&acc.email));
    }

    let handle = app_model::start(
        target,
        services.clone(),
        creds,
        store,
        "1.0.1".into(),
        false,
    );

    // 1. All succeed -> max concurrency <= 3, message "额度已刷新"
    handle.send(Command::RefreshQuotas);
    let s = wait_until(&handle, |s| {
        !s.refreshing && s.feedback.as_ref().is_some_and(|f| f.text == "额度已刷新")
    })
    .await;
    assert!(services.quota_max_active.load(Ordering::SeqCst) <= 3);
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Success
    ));
    assert_eq!(s.accounts[0].used_days, Some(2));
    assert_eq!(s.accounts[0].total_days, Some(5));

    // 2. All fail -> "额度更新失败"
    *services.quota_delay.lock().unwrap() = Duration::ZERO;
    *services.quota_result.lock().unwrap() = QuotaResult::Expired;
    handle.send(Command::RefreshQuotas);
    let s = wait_until(&handle, |s| {
        !s.refreshing
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text == "额度更新失败")
    })
    .await;
    assert!(s
        .accounts
        .iter()
        .all(|a| a.auth_state == AuthState::Expired));

    // 3. Partial failure: 4 accounts ok, 1 account missing credential -> "额度已刷新，1 个账号更新失败"
    *services.quota_result.lock().unwrap() = QuotaResult::Quota {
        used: 3,
        total: 5,
        plan: Some("Pro".into()),
    };
    // Remove credential for accounts[4]
    let creds2_dir = TempDataDir::new();
    let two = vec![accounts[0].clone(), accounts[1].clone()];
    let store2 = seed_store(&creds2_dir, two.clone());
    let creds2 = Arc::new(MemoryCredentialStore::default());
    creds2.insert_snapshot(two[0].id, &make_snapshot(&two[0].email));
    // two[1] intentionally has no credential -> Missing -> Expired -> 1 failure
    let services2 = Arc::new(FakeServices::default());
    let handle2 = app_model::start(
        Arc::new(FakeTarget::default()),
        services2,
        creds2,
        store2,
        "1.0.1".into(),
        false,
    );
    handle2.send(Command::RefreshQuotas);
    let s = wait_until(&handle2, |s| {
        !s.refreshing
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text == "额度已刷新，1 个账号更新失败")
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));
    assert_eq!(s.accounts[0].auth_state, AuthState::Ok);
    assert_eq!(s.accounts[1].auth_state, AuthState::Expired);
}

// ---------------------------------------------------------------------------
// 网页登录: C-WL-01 ~ C-WL-05 & I-007 关窗取消
// ---------------------------------------------------------------------------

#[test]
fn c_wl_01_to_04_callback_code_validation() {
    // C-WL-01
    assert_eq!(
        callback_code(
            "GET /callback?code=test&state=expected HTTP/1.1\r\n\r\n",
            "expected"
        ),
        Some("test".into())
    );
    // C-WL-02
    assert_eq!(
        callback_code(
            "GET /callback?code=test&state=wrong HTTP/1.1\r\n\r\n",
            "expected"
        ),
        None
    );
    // C-WL-03: duplicate state / code
    assert_eq!(
        callback_code(
            "GET /callback?code=test&state=expected&state=wrong HTTP/1.1\r\n\r\n",
            "expected"
        ),
        None
    );
    assert_eq!(
        callback_code(
            "GET /callback?code=a&code=b&state=expected HTTP/1.1\r\n\r\n",
            "expected"
        ),
        None
    );
    // C-WL-04: wrong path or non-GET method
    assert_eq!(
        callback_code(
            "GET /other?code=test&state=expected HTTP/1.1\r\n\r\n",
            "expected"
        ),
        None
    );
    assert_eq!(
        callback_code(
            "POST /callback?code=test&state=expected HTTP/1.1\r\n\r\n",
            "expected"
        ),
        None
    );
    assert_eq!(
        callback_code(
            "GET /callback?code=&state=expected HTTP/1.1\r\n\r\n",
            "expected"
        ),
        None
    );
}

#[tokio::test]
async fn i_007_auth_window_close_cancels_login_without_deadlock() {
    // Verify AuthWindow::open + closed() select semantics fixed in I-007
    use switchboard_core::web_login::AuthWindow;
    let window = Arc::new(FakeAuthWindow::default());
    window.open("https://pen.dev/auth").unwrap();
    let w2 = window.clone();
    let finisher = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        w2.close_notify.notify_waiters();
    });
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("closed() should resolve promptly"),
        res = window.closed() => assert!(res.is_ok()),
    }
    finisher.await.unwrap();
}

#[tokio::test]
async fn c_wl_04_05_and_i007_full_offline_web_login_flow() {
    async fn spawn_mock_api(token_delay: Duration) -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let redirects = Arc::new(std::sync::Mutex::new(Vec::new()));
        let redirects_bg = redirects.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let redirects = redirects_bg.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 4096];
                    loop {
                        let n = stream.read(&mut tmp).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&buf[..pos]);
                            let len = head
                                .lines()
                                .find_map(|l| {
                                    let (k, v) = l.split_once(':')?;
                                    k.eq_ignore_ascii_case("content-length")
                                        .then(|| v.trim().parse::<usize>().ok())
                                        .flatten()
                                })
                                .unwrap_or(0);
                            if buf.len() >= pos + 4 + len {
                                break;
                            }
                        }
                    }
                    let req = String::from_utf8_lossy(&buf);
                    let first_line = req.lines().next().unwrap_or("");
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("");
                    let reply_json = if first_line.starts_with("POST /auth/device-auth/start ") {
                        let v: serde_json::Value =
                            serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
                        if let Some(uri) = v["redirectUri"].as_str() {
                            redirects.lock().unwrap().push(uri.to_string());
                        }
                        r#"{"loginUrl":"/login?source=desktop"}"#.to_string()
                    } else if first_line.starts_with("POST /auth/device-auth/token ") {
                        if !token_delay.is_zero() {
                            tokio::time::sleep(token_delay).await;
                        }
                        r#"{"email":"oauth@example.test","sessionToken":"oauth-tok","plan":"Pro"}"#
                            .to_string()
                    } else {
                        "{}".to_string()
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        reply_json.len(),
                        reply_json
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                });
            }
        });
        (port, redirects)
    }

    async fn send_raw(port: u16, req: &[u8]) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let _ = stream.write_all(req).await;
        let mut resp = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            match stream.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => resp.extend_from_slice(&buf[..n]),
            }
        }
        String::from_utf8_lossy(&resp).into_owned()
    }

    // 1. Happy path + C-WL-04 (>16KB -> 400) + C-WL-05 (duplicate callback during /token -> 400)
    let (api_port, redirects) = spawn_mock_api(Duration::from_millis(180)).await;
    let window = Arc::new(FakeAuthWindow::default());
    let login = Arc::new(WebLogin::with_api_base(
        SharedAuthWindow(window.clone()),
        reqwest::Url::parse(&format!("http://127.0.0.1:{api_port}/")).unwrap(),
    ));
    let login_task = {
        let login = login.clone();
        tokio::spawn(async move { login.sign_in("dev-id", "darwin").await })
    };

    window.open_notify.notified().await;
    let opened = window.opened_urls.lock().unwrap().last().unwrap().clone();
    let state = reqwest::Url::parse(&opened)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let redirect_uri = redirects.lock().unwrap().last().unwrap().clone();
    let cb_port = reqwest::Url::parse(&redirect_uri).unwrap().port().unwrap();

    // C-WL-04: oversized request (>16KB) gets HTTP 400
    let pad = "a".repeat(17_000);
    let huge = format!("GET /callback?code=x&state={state}&pad={pad} HTTP/1.1\r\n\r\n");
    let huge_resp = send_raw(cb_port, huge.as_bytes()).await;
    assert!(huge_resp.starts_with("HTTP/1.1 400 Bad Request"));

    // C-WL-04: wrong path gets HTTP 400
    let wrong_path = send_raw(
        cb_port,
        format!("GET /wrong?code=x&state={state} HTTP/1.1\r\n\r\n").as_bytes(),
    )
    .await;
    assert!(wrong_path.starts_with("HTTP/1.1 400 Bad Request"));

    // C-WL-01: valid callback gets HTTP 200
    let ok_resp = send_raw(
        cb_port,
        format!("GET /callback?code=good-code&state={state} HTTP/1.1\r\n\r\n").as_bytes(),
    )
    .await;
    assert!(ok_resp.starts_with("HTTP/1.1 200 OK"));

    // C-WL-05: duplicate callback while POST /token is in flight gets HTTP 400
    let dup_resp = send_raw(
        cb_port,
        format!("GET /callback?code=second&state={state} HTTP/1.1\r\n\r\n").as_bytes(),
    )
    .await;
    assert!(dup_resp.starts_with("HTTP/1.1 400 Bad Request"));

    let snap = login_task.await.unwrap().unwrap();
    assert_eq!(snap.email, "oauth@example.test");
    assert_eq!(snap.token.as_deref(), Some("oauth-tok"));
    // Listener is dropped after sign_in completes
    assert!(TcpStream::connect(("127.0.0.1", cb_port)).await.is_err());

    // 2. Real sign_in + closing auth window cancels immediately without waiting for timeout
    let window2 = Arc::new(FakeAuthWindow::default());
    let login2 = Arc::new(WebLogin::with_api_base(
        SharedAuthWindow(window2.clone()),
        reqwest::Url::parse(&format!("http://127.0.0.1:{api_port}/")).unwrap(),
    ));
    let login2_task = {
        let login2 = login2.clone();
        tokio::spawn(async move { login2.sign_in("dev-id", "darwin").await })
    };
    window2.open_notify.notified().await;
    window2.close_notify.notify_waiters();
    let err = login2_task.await.unwrap().unwrap_err();
    assert_eq!(err, "网页登录已取消或超时，原账号保持不变");
}

// ---------------------------------------------------------------------------
// 更新: C-UP-01 ~ C-UP-05, C-UP-07 ~ C-UP-10, C-UP-12, C-UP-13
// ---------------------------------------------------------------------------

#[tokio::test]
async fn c_up_01_to_05_and_08_update_state_machine_signature_and_backoff() {
    let dir = TempDataDir::new();
    let store = seed_store(&dir, vec![]);
    let target = Arc::new(FakeTarget::default());
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());

    let release_v110 = AppRelease {
        version: "1.1.0".into(),
        asset_url: "https://example.test/app.zip".into(),
        signature_url: "https://example.test/app.zip.sig".into(),
        page_url: "https://example.test/rel".into(),
    };

    let handle = app_model::start(
        target,
        services.clone(),
        creds,
        store,
        "1.0.1".into(),
        false,
    );

    // C-UP-02: same version stays idle
    *services.feed_result.lock().unwrap() = Ok(Some(AppRelease {
        version: "1.0.1".into(),
        ..release_v110.clone()
    }));
    handle.send(Command::CheckUpdates(true));
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text == "已是最新版本 1.0.1")
    })
    .await;
    assert!(matches!(s.update, UpdateState::Idle));

    // C-UP-08: failed automatic check backs off for 1h, forced check ignores backoff
    let dir_backoff = TempDataDir::new();
    let store_backoff = seed_store(&dir_backoff, vec![]);
    let services_backoff = Arc::new(FakeServices::default());
    *services_backoff.feed_result.lock().unwrap() = Err("offline".into());
    let h_backoff = app_model::start(
        Arc::new(FakeTarget::default()),
        services_backoff.clone(),
        Arc::new(MemoryCredentialStore::default()),
        store_backoff,
        "1.0.1".into(),
        false,
    );
    h_backoff.send(Command::CheckUpdates(false));
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(services_backoff.feed_calls.load(Ordering::SeqCst), 1);

    h_backoff.send(Command::CheckUpdates(false));
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(services_backoff.feed_calls.load(Ordering::SeqCst), 1);

    h_backoff.send(Command::CheckUpdates(true));
    wait_until(&h_backoff, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text.contains("检查更新失败"))
    })
    .await;
    assert_eq!(services_backoff.feed_calls.load(Ordering::SeqCst), 2);

    // C-UP-01: newer release becomes visible Available
    *services.feed_result.lock().unwrap() = Ok(Some(release_v110.clone()));
    handle.send(Command::CheckUpdates(true));
    let s = wait_until(&handle, |s| {
        matches!(s.update, UpdateState::Available { .. })
    })
    .await;
    assert!(matches!(
        s.update,
        UpdateState::Available {
            silenced: false,
            ..
        }
    ));

    // Postpone silences banner while keeping Available
    handle.send(Command::PostponeUpdate);
    let s = wait_until(&handle, |s| {
        matches!(s.update, UpdateState::Available { silenced: true, .. })
    })
    .await;
    assert!(matches!(
        s.update,
        UpdateState::Available { silenced: true, .. }
    ));

    // C-UP-04 & C-UP-05: bad signature rejects before prepare_update (unpack)
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let sig_from_other_key = STANDARD.encode(signing_key.sign(b"zip-bytes").to_bytes());
    *services.signature_value.lock().unwrap() = Ok(sig_from_other_key);
    handle.send(Command::StartUpdate);
    let s = wait_until(&handle, |s| {
        matches!(s.update, UpdateState::Available { .. })
            && s.feedback
                .as_ref()
                .is_some_and(|f| f.text.contains("签名校验失败"))
    })
    .await;
    assert_eq!(services.prepare_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));

    // C-UP-05: missing signature_url rejects before download & unpack
    *services.feed_result.lock().unwrap() = Ok(Some(AppRelease {
        version: "1.2.0".into(),
        signature_url: "".into(),
        ..release_v110.clone()
    }));
    handle.send(Command::CheckUpdates(true));
    wait_until(&handle, |s| {
        matches!(&s.update, UpdateState::Available { release, .. } if release.version == "1.2.0")
    })
    .await;
    let dl_before = services.download_calls.load(Ordering::SeqCst);
    handle.send(Command::StartUpdate);
    wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text.contains("发布缺少签名文件"))
    })
    .await;
    assert_eq!(services.download_calls.load(Ordering::SeqCst), dl_before);
    assert_eq!(services.prepare_calls.load(Ordering::SeqCst), 0);

    // C-UP-03: Downloading progress percent calculation
    *services.feed_result.lock().unwrap() = Ok(Some(release_v110));
    handle.send(Command::CheckUpdates(true));
    wait_until(&handle, |s| {
        matches!(&s.update, UpdateState::Available { release, .. } if release.version == "1.1.0")
    })
    .await;
    *services.download_chunks.lock().unwrap() = vec![vec![0u8; 42]];
    *services.download_total.lock().unwrap() = Some(100);
    *services.download_delay.lock().unwrap() = Duration::from_millis(80);
    // verify_signature will still fail after download, giving us a clean window to observe Downloading
    handle.send(Command::StartUpdate);
    let s = wait_until(&handle, |s| {
        matches!(
            s.update,
            UpdateState::Downloading {
                percent: 42,
                received: 42,
                total: Some(100),
                ..
            }
        )
    })
    .await;
    assert!(matches!(
        s.update,
        UpdateState::Downloading {
            percent: 42,
            received: 42,
            total: Some(100),
            ..
        }
    ));
    wait_until(&handle, |s| {
        matches!(s.update, UpdateState::Available { .. })
    })
    .await;

    handle.send(Command::OpenUpdatePage);
    handle.flush().await;
    assert_eq!(
        services.opened_pages.lock().unwrap().as_slice(),
        &["https://example.test/rel"]
    );
}

#[tokio::test]
async fn c_up_07_09_10_signed_ready_install_and_asset_selection() {
    // C-UP-07: end-to-end signed bundle transitions to Ready and triggers InstallUpdate
    let dir = TempDataDir::new();
    let store = seed_store(&dir, vec![]);
    let target = Arc::new(FakeTarget::default());
    let services = Arc::new(FakeServices::default());
    let creds = Arc::new(MemoryCredentialStore::default());

    let signing_key = SigningKey::from_bytes(&[11u8; 32]);
    let pkg = b"signed-update-archive";
    let sig = STANDARD.encode(signing_key.sign(pkg).to_bytes());
    *services.update_public_key.lock().unwrap() = Some(signing_key.verifying_key().to_bytes());
    *services.download_chunks.lock().unwrap() = vec![pkg.to_vec()];
    *services.signature_value.lock().unwrap() = Ok(sig);
    *services.prepare_result.lock().unwrap() = Ok("/tmp/prepared-1.3.0.app".into());
    *services.feed_result.lock().unwrap() = Ok(Some(AppRelease {
        version: "1.3.0".into(),
        asset_url: "https://example.test/app-1.3.0-arm64.zip".into(),
        signature_url: "https://example.test/app-1.3.0-arm64.zip.sig".into(),
        page_url: "https://example.test/v1.3.0".into(),
    }));

    let handle = app_model::start(
        target,
        services.clone(),
        creds,
        store,
        "1.0.1".into(),
        false,
    );
    handle.send(Command::CheckUpdates(true));
    wait_until(&handle, |s| {
        matches!(&s.update, UpdateState::Available { release, .. } if release.version == "1.3.0")
    })
    .await;

    handle.send(Command::StartUpdate);
    let s = wait_until(&handle, |s| matches!(s.update, UpdateState::Ready { .. })).await;
    assert_eq!(services.prepare_calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        &s.update,
        UpdateState::Ready { release, prepared } if release.version == "1.3.0" && prepared == "/tmp/prepared-1.3.0.app"
    ));

    handle.send(Command::InstallUpdate);
    handle.flush().await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(services.install_calls.load(Ordering::SeqCst), 1);

    *services.install_result.lock().unwrap() = Err("需要写入权限".into());
    handle.send(Command::InstallUpdate);
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text == "安装更新失败：需要写入权限")
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));

    // C-UP-09 & C-UP-10: parse_release asset selection for Mac and Windows
    let mixed_release = serde_json::json!({
        "tag_name": "v1.3.0",
        "html_url": "https://github.com/example/repo/releases/tag/v1.3.0",
        "prerelease": false,
        "assets": [
            {"name": "PencilSwitcher-1.3.0.zip", "browser_download_url": "https://dl/generic.zip"},
            {"name": "PencilSwitcher-1.3.0.zip.sig", "browser_download_url": "https://dl/generic.zip.sig"},
            {"name": "PencilSwitcher-1.3.0-windows-x64-setup.exe", "browser_download_url": "https://dl/win.exe"},
            {"name": "PencilSwitcher-1.3.0-windows-x64-setup.exe.sig", "browser_download_url": "https://dl/win.exe.sig"},
            {"name": "PencilSwitcher-1.3.0-arm64.zip", "browser_download_url": "https://dl/arm64.zip"},
            {"name": "PencilSwitcher-1.3.0-arm64.zip.sig", "browser_download_url": "https://dl/arm64.zip.sig"}
        ]
    });
    let raw_mixed = serde_json::to_vec(&mixed_release).unwrap();

    let mac_rel = update::parse_release(&raw_mixed, PlatformAsset::Mac).unwrap();
    assert_eq!(mac_rel.version, "1.3.0");
    assert_eq!(mac_rel.asset_url, "https://dl/arm64.zip");
    assert_eq!(mac_rel.signature_url, "https://dl/arm64.zip.sig");

    let win_rel = update::parse_release(&raw_mixed, PlatformAsset::Windows).unwrap();
    assert_eq!(win_rel.version, "1.3.0");
    assert_eq!(win_rel.asset_url, "https://dl/win.exe");
    assert_eq!(win_rel.signature_url, "https://dl/win.exe.sig");

    // Mac rejects non-arm64 generic zip when arm64 zip is absent
    let generic_only = serde_json::to_vec(&serde_json::json!({
        "tag_name": "v1.3.0",
        "html_url": "https://example.test/r",
        "prerelease": false,
        "assets": [
            {"name": "PencilSwitcher-1.3.0.zip", "browser_download_url": "https://dl/generic.zip"},
            {"name": "PencilSwitcher-1.3.0.zip.sig", "browser_download_url": "https://dl/generic.zip.sig"},
            {"name": "PencilSwitcher-1.3.0-windows-x64-setup.exe", "browser_download_url": "https://dl/win.exe"},
            {"name": "PencilSwitcher-1.3.0-windows-x64-setup.exe.sig", "browser_download_url": "https://dl/win.exe.sig"}
        ]
    }))
    .unwrap();
    assert_eq!(
        update::parse_release(&generic_only, PlatformAsset::Mac).unwrap_err(),
        "发布缺少更新包"
    );
    assert!(update::parse_release(&generic_only, PlatformAsset::Windows).is_ok());

    // Missing .sig asset is rejected on both platforms
    let no_sig = serde_json::to_vec(&serde_json::json!({
        "tag_name": "v1.3.0",
        "html_url": "https://example.test/r",
        "prerelease": false,
        "assets": [
            {"name": "PencilSwitcher-1.3.0-arm64.zip", "browser_download_url": "https://dl/arm64.zip"},
            {"name": "PencilSwitcher-1.3.0-windows-x64-setup.exe", "browser_download_url": "https://dl/win.exe"}
        ]
    }))
    .unwrap();
    assert_eq!(
        update::parse_release(&no_sig, PlatformAsset::Mac).unwrap_err(),
        "发布缺少签名文件"
    );
    assert_eq!(
        update::parse_release(&no_sig, PlatformAsset::Windows).unwrap_err(),
        "发布缺少签名文件"
    );

    // Prerelease is rejected
    let pre = serde_json::to_vec(&serde_json::json!({
        "tag_name": "v1.3.0-rc.1",
        "html_url": "https://example.test/r",
        "prerelease": true,
        "assets": []
    }))
    .unwrap();
    assert_eq!(
        update::parse_release(&pre, PlatformAsset::Mac).unwrap_err(),
        "发布版本无效"
    );
}

#[test]
fn c_up_version_compare_and_signature_unit_checks() {
    assert_eq!(update::normalize(" v1.2.0 "), "1.2.0");
    assert!(update::compare("1.0.0", "1.1.0").is_lt());
    assert!(update::compare("1.1.0", "1.1.0").is_eq());
    assert!(update::compare("1.2.0", "1.10.0").is_lt());

    // C-UP-05: invalid base64 or wrong-key Ed25519 signature rejected
    assert_eq!(
        update::verify_signature(b"payload", "not-valid-base64").unwrap_err(),
        "更新包签名校验失败，已中止安装"
    );
    let key = SigningKey::from_bytes(&[9u8; 32]);
    let sig = STANDARD.encode(key.sign(b"payload").to_bytes());
    assert_eq!(
        update::verify_signature(b"payload", &sig).unwrap_err(),
        "更新包签名校验失败，已中止安装"
    );

    // UpdateSchedule state.json backoff checks
    let dir = TempDataDir::new();
    let mut sched = UpdateSchedule::new(&dir);
    assert!(sched.should_check(false));
    sched.attempt().unwrap();
    assert!(!sched.should_check(false));
    assert!(sched.should_check(true));
    let raw: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("state.json")).unwrap()).unwrap();
    assert!(raw["lastAttempt"].as_u64().is_some());

    // Simulate > 1h since last_attempt and no last_check
    let two_hours_ago = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 7200;
    atomic_write(
        &dir.path().join("state.json"),
        format!(r#"{{"lastAttempt":{two_hours_ago}}}"#).as_bytes(),
        false,
    )
    .unwrap();
    let sched2 = UpdateSchedule::new(&dir);
    assert!(sched2.should_check(false));
    let mut sched3 = UpdateSchedule::new(&dir);
    sched3.success().unwrap();
    assert!(!sched3.should_check(false));
    assert!(sched3.should_check(true));
    let raw2: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("state.json")).unwrap()).unwrap();
    assert!(raw2["lastCheck"].as_u64().is_some());
}

#[tokio::test]
async fn c_up_12_13_download_http_404_200_and_cancel() {
    async fn spawn_one_shot(status: u16, body: &'static [u8], delay: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                let reason = if status < 400 { "OK" } else { "ERR" };
                let head = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(body).await;
            }
        });
        format!("http://127.0.0.1:{port}/file")
    }

    let client = reqwest::Client::new();

    // C-UP-12: HTTP 404 includes status code
    let url_404 = spawn_one_shot(404, b"missing", Duration::ZERO).await;
    let err = update::download(&client, &url_404, &CancellationToken::new(), |_, _| {})
        .await
        .unwrap_err();
    assert!(err.contains("HTTP 404"), "unexpected error: {err}");

    // C-UP-12: HTTP 200 preserves exact payload and reports progress
    let url_200 = spawn_one_shot(200, b"zip-bytes", Duration::ZERO).await;
    let last_progress = std::sync::Mutex::new((0, None));
    let data = update::download(&client, &url_200, &CancellationToken::new(), |r, t| {
        *last_progress.lock().unwrap() = (r, t);
    })
    .await
    .unwrap();
    assert_eq!(data, b"zip-bytes");
    assert_eq!(*last_progress.lock().unwrap(), (9, Some(9)));

    // C-UP-13: cancelling stops slow download promptly
    let url_slow = spawn_one_shot(200, b"slow", Duration::from_secs(3)).await;
    let token = CancellationToken::new();
    let cancel_token = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_token.cancel();
    });
    let start = tokio::time::Instant::now();
    let err = update::download(&client, &url_slow, &token, |_, _| {})
        .await
        .unwrap_err();
    assert_eq!(err, "已取消");
    assert!(start.elapsed() < Duration::from_secs(2));
}

// ---------------------------------------------------------------------------
// 演示模式 & 22 个视觉快照场景
// ---------------------------------------------------------------------------

#[tokio::test]
async fn demo_mode_and_22_snapshot_scenarios() {
    let dir = TempDataDir::new();
    let store = Store::new(&dir);
    let handle = app_model::start(
        Arc::new(FakeTarget::default()),
        Arc::new(FakeServices::default()),
        Arc::new(MemoryCredentialStore::default()),
        store,
        "1.0.1".into(),
        true,
    );
    let s = handle.state();
    assert!(s.demo);
    assert_eq!(s.accounts.len(), 3);

    handle.send(Command::AddAccount);
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text == "演示模式不操作真实账号")
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));

    handle.send(Command::RefreshQuotas);
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text == "演示模式不请求真实额度")
    })
    .await;
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));

    let demo_id = s.accounts[1].id;
    handle.send(Command::RemoveAccount(demo_id));
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text == "演示模式不操作真实账号")
    })
    .await;
    assert_eq!(s.accounts.len(), 3);

    handle.send(Command::CheckUpdates(false));
    handle.flush().await;
    assert!(matches!(handle.state().update, UpdateState::Idle));

    assert!(snapshot::scenario("main").is_some());
    assert!(snapshot::scenario("settings").is_some());
    assert!(snapshot::scenario("settings-launch-on").is_some());
    assert!(snapshot::scenario("manage").is_some());
    assert!(snapshot::scenario("main-empty").is_some());
    assert!(snapshot::scenario("main-single").is_some());
    assert!(snapshot::scenario("main-unregistered-live").is_some());
    assert!(snapshot::scenario("main-expired").is_some());
    assert!(snapshot::scenario("main-unknown-quota").is_some());
    assert!(snapshot::scenario("main-switching").is_some());
    assert!(snapshot::scenario("main-refreshing").is_some());
    assert!(snapshot::scenario("main-adding").is_some());
    assert!(snapshot::scenario("main-feedback-success").is_some());
    assert!(snapshot::scenario("main-feedback-warning").is_some());
    assert!(snapshot::scenario("main-feedback-long").is_some());
    assert!(snapshot::scenario("main-persistence-blocked").is_some());
    assert!(snapshot::scenario("main-update-available").is_some());
    assert!(snapshot::scenario("main-update-downloading").is_some());
    assert!(snapshot::scenario("main-update-ready").is_some());
    assert!(snapshot::scenario("settings-update-available").is_some());
    assert!(snapshot::scenario("manage-expired").is_some());
    assert!(snapshot::scenario("manage-confirm-remove").is_some());
    assert_eq!(snapshot::scenario("main").unwrap().route, "main");
    assert_eq!(snapshot::scenario("settings").unwrap().route, "settings");
    assert!(
        snapshot::scenario("settings-launch-on")
            .unwrap()
            .state
            .launch_at_login
    );
    assert_eq!(snapshot::scenario("manage").unwrap().route, "manage");
    assert!(snapshot::scenario("main-empty")
        .unwrap()
        .state
        .accounts
        .is_empty());
    assert!(
        snapshot::scenario("main-persistence-blocked")
            .unwrap()
            .state
            .persistence_blocked
    );
    assert!(snapshot::scenario("manage-confirm-remove")
        .unwrap()
        .confirm_remove_email
        .is_some());
    assert!(snapshot::scenario("does-not-exist").is_none());
}

#[tokio::test]
async fn settings_launch_at_login_and_empty_update_repo() {
    let dir = TempDataDir::new();
    let store = seed_store(&dir, vec![]);
    let target = Arc::new(FakeTarget::default());
    let services = Arc::new(FakeServices::default());
    *services.update_repo.lock().unwrap() = String::new();
    let creds = Arc::new(MemoryCredentialStore::default());

    let handle = app_model::start(
        target,
        services.clone(),
        creds,
        store,
        "1.0.1".into(),
        false,
    );

    // Empty update_repo skips feed check
    handle.send(Command::CheckUpdates(true));
    handle.flush().await;
    assert_eq!(services.feed_calls.load(Ordering::SeqCst), 0);

    // Launch at login succeeds and updates ViewState
    handle.send(Command::SetLaunchAtLogin(true));
    let s = wait_until(&handle, |s| s.launch_at_login).await;
    assert!(s.launch_at_login);

    // Launch at login failure keeps previous state and surfaces warning feedback
    *services.set_launch_error.lock().unwrap() = Some("权限不足".into());
    handle.send(Command::SetLaunchAtLogin(false));
    let s = wait_until(&handle, |s| {
        s.feedback
            .as_ref()
            .is_some_and(|f| f.text.contains("权限不足"))
    })
    .await;
    assert!(s.launch_at_login);
    assert!(matches!(
        s.feedback.as_ref().unwrap().kind,
        FeedbackKind::Warning
    ));
}
