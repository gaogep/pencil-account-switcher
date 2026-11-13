use std::{path::Path, sync::Arc};
use switchboard_core::{
    app_model::AppServices,
    quota::{PenQuotaProvider, QuotaResult},
    target::{TargetFuture, TargetSnapshot},
    update::PlatformAsset,
    web_login::WebLogin,
};
use switchboard_platform::{
    macos::{MacAuthWindow, MacPlatform},
    Platform,
};

pub struct Services {
    pub quota: PenQuotaProvider<MacPlatform>,
    pub login: WebLogin<MacAuthWindow>,
    pub app: tauri::AppHandle,
}
impl Services {
    pub fn new(app: tauri::AppHandle) -> Arc<Self> {
        Arc::new(Self {
            quota: PenQuotaProvider::new("https://api.pencil.dev".into(), MacPlatform),
            login: WebLogin::new(MacAuthWindow::default()),
            app,
        })
    }
}
impl AppServices for Services {
    fn login(&self) -> TargetFuture<'_, Result<TargetSnapshot, String>> {
        Box::pin(self.login.sign_in(self.quota.device_id(), "darwin"))
    }
    fn cancel_login(&self) {
        self.login.cancel();
    }
    fn quota<'a>(&'a self, snapshot: &'a TargetSnapshot) -> TargetFuture<'a, QuotaResult> {
        Box::pin(self.quota.fetch_quota(snapshot))
    }
    fn launch_at_login(&self) -> Result<bool, String> {
        MacPlatform.launch_at_login().map_err(|e| e.to_string())
    }
    fn set_launch_at_login(&self, enabled: bool) -> Result<(), String> {
        MacPlatform
            .set_launch_at_login(enabled)
            .map_err(|e| e.to_string())
    }
    // Automatic updating is enabled when the platform installer arrives in P5.
    fn update_repo(&self) -> &str {
        ""
    }
    fn update_platform(&self) -> PlatformAsset {
        PlatformAsset::Mac
    }
    fn prepare_update<'a>(
        &'a self,
        _: &'a Path,
        _: &'a str,
    ) -> TargetFuture<'a, Result<String, String>> {
        Box::pin(async { Err("更新安装器尚未接入".into()) })
    }
    fn install_update<'a>(&'a self, _: &'a str) -> TargetFuture<'a, Result<(), String>> {
        Box::pin(async { Err("更新安装器尚未接入".into()) })
    }
    fn open_update_page(&self, url: &str) -> Result<(), String> {
        let url = url.to_string();
        self.app
            .run_on_main_thread(move || {
                use objc2_foundation::{NSString, NSURL};
                if let Some(url) = NSURL::URLWithString(&NSString::from_str(&url)) {
                    objc2_app_kit::NSWorkspace::sharedWorkspace().openURL(&url);
                }
            })
            .map_err(|e| e.to_string())
    }
}
