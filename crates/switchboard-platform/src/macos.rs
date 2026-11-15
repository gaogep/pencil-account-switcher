use crate::Platform;
use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_service_management::{SMAppService, SMAppServiceStatus};
use std::{
    io,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
use switchboard_core::{
    pen_adapter::PenPlatform, quota::DeviceIdentity, store::DataDirectory, target::TargetFuture,
};

pub(super) fn on_main<F, R>(work: F) -> R
where
    F: FnOnce(MainThreadMarker) -> R + Send,
    R: Send,
{
    if let Some(marker) = MainThreadMarker::new() {
        return work(marker);
    }
    let (tx, rx) = mpsc::sync_channel(1);
    dispatch2::DispatchQueue::main().exec_sync(move || {
        let marker = MainThreadMarker::new().expect("main dispatch queue");
        let _ = tx.send(work(marker));
    });
    rx.recv().expect("main queue returned a result")
}

#[derive(Clone, Copy, Default)]
pub struct MacPlatform;
impl DataDirectory for MacPlatform {
    fn data_dir(&self) -> PathBuf {
        PathBuf::from(std::env::var_os("HOME").expect("macOS user home"))
            .join("Library/Application Support/Switchboard")
    }
}
impl Platform for MacPlatform {
    fn os_tag(&self) -> &'static str {
        "darwin"
    }
    fn find_target_processes(&self) -> Vec<u32> {
        on_main(|_| {
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
                "dev.pencil.desktop",
            ))
            .iter()
            .filter(|app| !app.isTerminated())
            .map(|app| app.processIdentifier() as u32)
            .collect()
        })
    }
    fn request_quit(&self, pids: &[u32]) {
        on_main(|_| {
            for app in NSRunningApplication::runningApplicationsWithBundleIdentifier(
                &NSString::from_str("dev.pencil.desktop"),
            )
            .iter()
            {
                if pids.contains(&(app.processIdentifier() as u32)) {
                    app.terminate();
                }
            }
        });
    }
    fn resolve_target_app(&self) -> Option<PathBuf> {
        on_main(|_| {
            NSWorkspace::sharedWorkspace()
                .URLForApplicationWithBundleIdentifier(&NSString::from_str("dev.pencil.desktop"))
                .and_then(|url| url.path())
                .map(|p| PathBuf::from(p.to_string()))
        })
        .or_else(|| {
            let path = PathBuf::from("/Applications/Pen.app");
            path.exists().then_some(path)
        })
    }
    fn launch_target<'a>(&'a self, app: &'a Path) -> TargetFuture<'a, io::Result<()>> {
        Box::pin(async move {
            let path = app.to_string_lossy().into_owned();
            let (tx, rx) = tokio::sync::oneshot::channel();
            dispatch2::DispatchQueue::main().exec_async(move || {
                let tx = std::sync::Mutex::new(Some(tx));
                let callback =
                    RcBlock::new(move |_: *mut NSRunningApplication, error: *mut NSError| {
                        let result = unsafe { error.as_ref() }.map_or(Ok(()), |e| {
                            Err(io::Error::other(e.localizedDescription().to_string()))
                        });
                        if let Some(tx) = tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
                            let _ = tx.send(result);
                        }
                    });
                NSWorkspace::sharedWorkspace()
                    .openApplicationAtURL_configuration_completionHandler(
                        &NSURL::fileURLWithPath(&NSString::from_str(&path)),
                        &NSWorkspaceOpenConfiguration::configuration(),
                        Some(&callback),
                    );
            });
            rx.await.map_err(io::Error::other)?
        })
    }
    fn launch_at_login(&self) -> io::Result<bool> {
        on_main(|_| {
            Ok(unsafe { SMAppService::mainAppService().status() == SMAppServiceStatus::Enabled })
        })
    }
    fn set_launch_at_login(&self, on: bool) -> io::Result<()> {
        on_main(|_| unsafe {
            let service = SMAppService::mainAppService();
            let result = if on {
                service.registerAndReturnError()
            } else {
                service.unregisterAndReturnError()
            };
            result.map_err(|e| io::Error::other(e.localizedDescription().to_string()))
        })
    }
    fn restrict_permissions(&self, path: &Path, dir: bool) -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if dir { 0o700 } else { 0o600 }),
        )
    }
}
impl PenPlatform for MacPlatform {
    fn is_running(&self) -> bool {
        !self.find_target_processes().is_empty()
    }
    fn terminate_and_wait(&self, timeout: Duration) -> TargetFuture<'_, bool> {
        Box::pin(async move {
            self.request_quit(&self.find_target_processes());
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                if !self.is_running() {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            !self.is_running()
        })
    }
    fn can_launch(&self) -> bool {
        self.resolve_target_app().is_some()
    }
    fn launch(&self) -> TargetFuture<'_, Result<(), String>> {
        Box::pin(async move {
            let app = self
                .resolve_target_app()
                .ok_or("找不到 Pen，请先安装后再切换")?;
            self.launch_target(&app).await.map_err(|e| e.to_string())
        })
    }
}
impl DeviceIdentity for MacPlatform {
    fn hostname(&self) -> String {
        let mut buffer = [0u8; 256];
        if unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) } == 0 {
            let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            String::from_utf8_lossy(&buffer[..end]).into_owned()
        } else {
            on_main(|_| {
                objc2_foundation::NSProcessInfo::processInfo()
                    .hostName()
                    .to_string()
            })
        }
    }
    fn os_tag(&self) -> &'static str {
        "darwin"
    }
    fn architecture(&self) -> &'static str {
        if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        }
    }
}

mod auth;
pub use auth::MacAuthWindow;
