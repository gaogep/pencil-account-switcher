use std::{path::Path, sync::Arc};
use switchboard_core::{
    app_model::AppServices,
    quota::{PenQuotaProvider, QuotaResult},
    target::{TargetFuture, TargetSnapshot},
    update::PlatformAsset,
    web_login::WebLogin,
};
use switchboard_platform::{update::NativeUpdateInstaller, windows::WindowsPlatform, Platform};

pub struct Services {
    installer: NativeUpdateInstaller,
    pub quota: PenQuotaProvider<WindowsPlatform>,
    pub login: WebLogin<super::auth::WindowsAuthWindow>,
    pub platform: WindowsPlatform,
    app: tauri::AppHandle,
}
impl Services {
    pub fn new(
        app: tauri::AppHandle,
        platform: WindowsPlatform,
    ) -> Result<Arc<Self>, std::io::Error> {
        let executable = std::env::current_exe()?;
        let current = executable;
        Ok(Arc::new(Self {
            installer: NativeUpdateInstaller::new(current),
            quota: PenQuotaProvider::new("https://api.pencil.dev".into(), platform.clone()),
            login: WebLogin::new(super::auth::WindowsAuthWindow::new(app.clone())),
            platform,
            app,
        }))
    }
}
impl AppServices for Services {
    fn login(&self) -> TargetFuture<'_, Result<TargetSnapshot, String>> {
        Box::pin(self.login.sign_in(self.quota.device_id(), "win32"))
    }
    fn cancel_login(&self) {
        self.login.cancel();
    }
    fn quota<'a>(&'a self, snapshot: &'a TargetSnapshot) -> TargetFuture<'a, QuotaResult> {
        Box::pin(self.quota.fetch_quota(snapshot))
    }
    fn launch_at_login(&self) -> Result<bool, String> {
        self.platform.launch_at_login().map_err(|e| e.to_string())
    }
    fn set_launch_at_login(&self, enabled: bool) -> Result<(), String> {
        self.platform
            .set_launch_at_login(enabled)
            .map_err(|e| e.to_string())
    }
    fn update_repo(&self) -> &str {
        "gaogep/pencil-account-switcher"
    }
    fn update_platform(&self) -> PlatformAsset {
        PlatformAsset::Windows
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
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        let url: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let action: Vec<u16> = "open\0".encode_utf16().collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                action.as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            Err("无法打开更新页面".into())
        } else {
            Ok(())
        }
    }
}
