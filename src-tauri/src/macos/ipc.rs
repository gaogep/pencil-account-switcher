use super::{
    panel::{self, PanelState},
    quit,
};
use switchboard_core::app_model::{Command, ViewState};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[tauri::command]
pub fn get_state(state: State<'_, PanelState>) -> ViewState {
    state.model.state()
}
macro_rules! command {
    ($name:ident, $variant:ident) => {
        #[tauri::command]
        pub fn $name(state: State<'_, PanelState>) {
            state.model.send(Command::$variant);
        }
    };
}
command!(add_account, AddAccount);
command!(cancel_add_account, CancelAddAccount);
command!(import_current_login, ImportCurrentLogin);
command!(refresh_quotas, RefreshQuotas);
command!(start_update, StartUpdate);
command!(postpone_update, PostponeUpdate);
command!(install_update, InstallUpdate);
command!(open_update_page, OpenUpdatePage);
#[tauri::command]
pub fn switch_to(state: State<'_, PanelState>, id: Uuid) {
    state.model.send(Command::SwitchTo(id));
}
#[tauri::command]
pub fn remove_account(state: State<'_, PanelState>, id: Uuid) {
    state.model.send(Command::RemoveAccount(id));
}
#[tauri::command]
pub fn set_launch_at_login(state: State<'_, PanelState>, enabled: bool) {
    state.model.send(Command::SetLaunchAtLogin(enabled));
}
#[tauri::command]
pub fn check_updates(state: State<'_, PanelState>, force: bool) {
    state.model.send(Command::CheckUpdates(force));
}
#[tauri::command]
pub fn hide_panel(app: AppHandle) {
    panel::hide(&app);
}
#[tauri::command]
pub fn set_panel_height(app: AppHandle, h: f64) -> Result<(), String> {
    panel::resize(&app, h)
}
#[tauri::command]
pub fn set_confirmation_open(state: State<'_, PanelState>, open: bool) {
    state.inner.lock().unwrap().confirmation_open = open;
}
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    quit(&app);
}

pub fn publish(app: &AppHandle) {
    let mut receiver = app.state::<PanelState>().model.subscribe();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let state = receiver.borrow_and_update().clone();
            if app.emit("state", state).is_err() {
                break;
            }
            if receiver.changed().await.is_err() {
                break;
            }
        }
    });
}
