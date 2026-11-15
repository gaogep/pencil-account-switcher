//! Serializable command contract shared with the frontend.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Serialize, Deserialize, TS)]
#[serde(tag = "command", content = "args", rename_all = "snake_case")]
pub enum IpcRequest {
    GetState,
    AddAccount,
    CancelAddAccount,
    ImportCurrentLogin,
    SwitchTo { id: Uuid },
    RemoveAccount { id: Uuid },
    RefreshQuotas,
    SetLaunchAtLogin { enabled: bool },
    CheckUpdates { force: bool },
    StartUpdate,
    PostponeUpdate,
    InstallUpdate,
    OpenUpdatePage,
    HidePanel,
    SetPanelHeight { h: f64 },
    SetConfirmationOpen { open: bool },
    QuitApp,
}
#[derive(TS)]
pub struct IpcEvents {
    pub state: crate::app_model::ViewState,
    #[ts(rename = "panel-shown")]
    pub panel_shown: (),
}
