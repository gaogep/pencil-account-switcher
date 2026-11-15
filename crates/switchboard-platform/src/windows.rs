//! Windows services. Pen is only asked to quit through its main window.
mod process;
use crate::Platform;
use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use switchboard_core::{
    pen_adapter::PenPlatform, quota::DeviceIdentity, store::DataDirectory, target::TargetFuture,
};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    RegKey,
};
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const STARTUP_NAME: &str = "Switchboard";
#[derive(Clone, Default)]
pub struct WindowsPlatform {
    remembered: Arc<Mutex<Vec<PathBuf>>>,
}
impl DataDirectory for WindowsPlatform {
    fn data_dir(&self) -> PathBuf {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("Windows local application data"))
            .join("Switchboard")
    }
}
fn startup_disabled() -> io::Result<bool> {
    let key = match RegKey::predef(HKEY_CURRENT_USER).open_subkey(APPROVED) {
        Ok(key) => key,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    match key.get_raw_value(STARTUP_NAME) {
        Ok(value) => Ok(matches!(value.bytes.first(), Some(3 | 7))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}
impl Platform for WindowsPlatform {
    fn os_tag(&self) -> &'static str {
        "win32"
    }
    fn find_target_processes(&self) -> Vec<u32> {
        process::processes()
            .unwrap_or_default()
            .iter()
            .map(|p| p.pid)
            .collect()
    }
    fn request_quit(&self, pids: &[u32]) {
        if let Ok(running) = process::processes() {
            let running: Vec<_> = running
                .into_iter()
                .filter(|p| pids.contains(&p.pid))
                .collect();
            *self.remembered.lock().unwrap() =
                running.iter().map(|p| PathBuf::from(&p.path)).collect();
            let _ = process::request_quit(&running);
        }
    }
    fn resolve_target_app(&self) -> Option<PathBuf> {
        if let Some(path) = self.remembered.lock().unwrap().iter().find(|p| p.is_file()) {
            return Some(path.clone());
        }
        if let Ok(root) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Uninstall")
        {
            for name in root.enum_keys().flatten() {
                let Ok(key) = root.open_subkey(name) else {
                    continue;
                };
                if key
                    .get_value::<String, _>("DisplayName")
                    .is_ok_and(|name| name.eq_ignore_ascii_case("Pen"))
                {
                    if let Ok(location) = key.get_value::<String, _>("InstallLocation") {
                        let path = PathBuf::from(location).join("Pen.exe");
                        if path.is_file() {
                            return Some(path);
                        }
                    }
                }
            }
        }
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
        ["Pen", "pen"]
            .into_iter()
            .map(|name| local.join("Programs").join(name).join("Pen.exe"))
            .find(|p| p.is_file())
    }
    fn launch_target<'a>(&'a self, app: &'a Path) -> TargetFuture<'a, io::Result<()>> {
        Box::pin(async move { process::launch(app).map_err(io::Error::other) })
    }
    fn launch_at_login(&self) -> io::Result<bool> {
        let key = match RegKey::predef(HKEY_CURRENT_USER).open_subkey(RUN) {
            Ok(key) => key,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        };
        let command: String = match key.get_value(STARTUP_NAME) {
            Ok(value) => value,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        };
        Ok(
            command == format!("\"{}\"", std::env::current_exe()?.display())
                && !startup_disabled()?,
        )
    }
    fn set_launch_at_login(&self, on: bool) -> io::Result<()> {
        if on && startup_disabled()? {
            return Err(io::Error::other(
                "开机自启已被 Windows 禁用，请在系统设置中启用",
            ));
        }
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey_with_flags(RUN, KEY_READ | KEY_WRITE)?;
        if on {
            key.set_value(
                STARTUP_NAME,
                &format!("\"{}\"", std::env::current_exe()?.display()),
            )
        } else {
            match key.delete_value(STARTUP_NAME) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        }
    }
    fn restrict_permissions(&self, _: &Path, _: bool) -> io::Result<()> {
        // The per-user LOCALAPPDATA directory supplies the inherited Windows ACL.
        Ok(())
    }
}
impl PenPlatform for WindowsPlatform {
    fn is_running(&self) -> bool {
        process::processes().map(|p| !p.is_empty()).unwrap_or(true)
    }
    fn terminate_and_wait(&self, timeout: Duration) -> TargetFuture<'_, bool> {
        Box::pin(async move {
            let Ok(running) = process::processes() else {
                return false;
            };
            if running.is_empty() {
                return true;
            }
            // Capture executable locations before WM_CLOSE removes the processes.
            *self.remembered.lock().unwrap() =
                running.iter().map(|p| PathBuf::from(&p.path)).collect();
            if process::request_quit(&running).is_err() {
                return false;
            }
            let deadline = Instant::now() + timeout;
            loop {
                match process::processes() {
                    Ok(p) if p.is_empty() => return true,
                    Err(_) => return false,
                    _ => {}
                }
                if Instant::now() >= deadline {
                    return false;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
    }
    fn can_launch(&self) -> bool {
        // Running portable installations may not have an Uninstall registry entry.
        if let Ok(running) = process::processes() {
            if !running.is_empty() {
                *self.remembered.lock().unwrap() =
                    running.iter().map(|p| PathBuf::from(&p.path)).collect();
            }
        }
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
impl DeviceIdentity for WindowsPlatform {
    fn hostname(&self) -> String {
        use windows_sys::Win32::Networking::WinSock::{
            GetHostNameW, WSACleanup, WSAStartup, WSADATA,
        };
        unsafe {
            let mut data: WSADATA = std::mem::zeroed();
            if WSAStartup(0x0202, &mut data) != 0 {
                return String::new();
            }
            let mut buffer = [0u16; 256];
            let result = GetHostNameW(buffer.as_mut_ptr(), buffer.len() as i32);
            WSACleanup();
            if result != 0 {
                return String::new();
            }
            let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
            String::from_utf16_lossy(&buffer[..end])
        }
    }
    fn os_tag(&self) -> &'static str {
        "win32"
    }
    fn architecture(&self) -> &'static str {
        "x64"
    }
}
