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
    update::NativeUpdateInstaller,
    Platform,
};

pub struct Services {
    installer: NativeUpdateInstaller,
    pub quota: PenQuotaProvider<MacPlatform>,
    pub login: WebLogin<MacAuthWindow>,
    pub app: tauri::AppHandle,
}
impl Services {
    pub fn new(app: tauri::AppHandle) -> Result<Arc<Self>, std::io::Error> {
        let executable = std::env::current_exe()?;
        let current = executable
            .ancestors()
            .nth(3)
            .unwrap_or(&executable)
            .to_path_buf();
        Ok(Arc::new(Self {
            installer: NativeUpdateInstaller::new(current),
            quota: PenQuotaProvider::new("https://api.pencil.dev".into(), MacPlatform),
            login: WebLogin::new(MacAuthWindow::default()),
            app,
        }))
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
    fn update_repo(&self) -> &str {
        "gaogep/pencil-account-switcher"
    }
    fn update_platform(&self) -> PlatformAsset {
        PlatformAsset::Mac
    }
    fn prepare_update<'a>(
        &'a self,
        package: &'a Path,
        version: &'a str,
    ) -> TargetFuture<'a, Result<String, String>> {
        Box::pin(self.installer.prepare(package, version))
    }
    fn install_update<'a>(&'a self, prepared: &'a str) -> TargetFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.installer.install(prepared)?;
            super::quit(&self.app);
            Ok(())
        })
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
