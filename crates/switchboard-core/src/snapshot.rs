use crate::{
    app_model::{Feedback, FeedbackKind, UpdateState, ViewState},
    model::{AccountRecord, AuthState},
    update::AppRelease,
};
use chrono::{Local, TimeZone};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub state: ViewState,
    pub route: &'static str,
    pub confirm_remove_email: Option<String>,
}

pub fn scenario(name: &str) -> Option<Snapshot> {
    let timestamp = Local
        .with_ymd_and_hms(2026, 11, 13, 10, 24, 0)
        .single()?
        .timestamp() as f64
        - 978_307_200.0;
    let accounts = [
        ("alex.chen@gmail.com", "Pro 方案", true, 1),
        ("jamie.work@outlook.com", "免费方案", false, 2),
        ("alex.creative@icloud.com", "团队方案", false, 4),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (email, plan, current, used))| AccountRecord {
        id: Uuid::from_u128((index + 1) as u128),
        email: email.into(),
        plan: Some(plan.into()),
        is_current: current,
        used_days: Some(used),
        total_days: Some(5),
        last_quota_update: Some(timestamp),
        last_error: None,
        auth_state: AuthState::Ok,
    })
    .collect();
    let mut snapshot = Snapshot {
        state: ViewState {
            accounts,
            live_email: None,
            unregistered_live_email: None,
            switching_to: None,
            refreshing: false,
            adding_account: false,
            feedback: None,
            update: UpdateState::Idle,
            launch_at_login: false,
            version: "1.0.1".into(),
            persistence_blocked: false,
            demo: true,
        },
        route: "main",
        confirm_remove_email: None,
    };
    let state = &mut snapshot.state;
    let release = || AppRelease {
        version: "1.1.0".into(),
        asset_url: "https://example.invalid/Pencil-Account-Switcher-1.1.0-arm64.zip".into(),
        signature_url: "https://example.invalid/Pencil-Account-Switcher-1.1.0-arm64.zip.sig".into(),
        page_url: "https://example.invalid/releases/v1.1.0".into(),
    };
    let feedback = |text: &str, kind, caption: Option<&str>| {
        Some(Feedback {
            text: text.into(),
            kind,
            caption: caption.map(str::to_string),
        })
    };
    match name {
        "main" => (),
        "settings" => snapshot.route = "settings",
        "settings-launch-on" => { state.launch_at_login = true; snapshot.route = "settings"; },
        "manage" => snapshot.route = "manage",
        "main-empty" => state.accounts.clear(),
        "main-single" => state.accounts.retain(|a| a.is_current),
        "main-unregistered-live" => { state.live_email = Some("new.member@example.com".into()); state.unregistered_live_email = state.live_email.clone(); },
        "main-expired" => { let a = &mut state.accounts[1]; a.auth_state = AuthState::Expired; a.last_error = Some("登录已失效".into()); a.used_days = None; a.total_days = None; },
        "main-unknown-quota" => for a in state.accounts.iter_mut().filter(|a| !a.is_current) { a.auth_state = AuthState::Unknown; a.used_days = None; a.total_days = None; a.last_quota_update = None; },
        "main-switching" => state.switching_to = Some(state.accounts[1].id),
        "main-refreshing" => { state.refreshing = true; state.feedback = feedback("正在刷新额度…", FeedbackKind::Loading, None); },
        "main-adding" => { state.adding_account = true; state.feedback = feedback("请在网页登录要添加的账号", FeedbackKind::Loading, None); },
        "main-feedback-success" => state.feedback = feedback("额度已刷新", FeedbackKind::Success, Some("刚刚更新")),
        "main-feedback-warning" => state.feedback = feedback("额度已刷新，1 个账号更新失败", FeedbackKind::Warning, None),
        "main-feedback-long" => state.feedback = feedback("已切换到 jamie.work@outlook.com，但 Pen 启动失败，请手动打开：找不到 Pen，请先安装后再切换", FeedbackKind::Warning, None),
        "main-persistence-blocked" => { state.persistence_blocked = true; state.feedback = feedback("账号列表已损坏，已备份且不会覆盖原文件", FeedbackKind::Warning, None); },
        "main-update-available" => state.update = UpdateState::Available { release: release(), silenced: false },
        "main-update-downloading" => state.update = UpdateState::Downloading { release: release(), percent: 42, received: 4_404_019, total: Some(10_485_760) },
        "main-update-ready" => state.update = UpdateState::Ready { release: release(), prepared: "/tmp/Pencil Account Switcher.app".into() },
        "settings-update-available" => { state.update = UpdateState::Available { release: release(), silenced: false }; snapshot.route = "settings"; },
        "manage-expired" => { state.accounts[2].auth_state = AuthState::Expired; snapshot.route = "manage"; },
        "manage-confirm-remove" => { snapshot.route = "manage"; snapshot.confirm_remove_email = Some(state.accounts[1].email.clone()); },
        _ => return None,
    }
    Some(snapshot)
}
