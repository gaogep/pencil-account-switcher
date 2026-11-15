//! Staging for packages already authenticated by switchboard-core.
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use tempfile::TempDir;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub struct NativeUpdateInstaller {
    current: PathBuf,
    prepared: Mutex<Option<Prepared>>,
}
struct Prepared {
    _directory: TempDir,
    item: PathBuf,
    _version: String,
}
impl NativeUpdateInstaller {
    pub fn new(current: PathBuf) -> Self {
        Self {
            current,
            prepared: Mutex::new(None),
        }
    }
    pub async fn prepare(&self, package: &Path, version: &str) -> Result<String, String> {
        let directory = tempfile::Builder::new()
            .prefix("SwitchboardUpdate-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        let item = macos::prepare(package, directory.path(), version).await?;
        #[cfg(target_os = "windows")]
        let item = windows::prepare(package, directory.path()).await?;
        let key = item.to_string_lossy().into_owned();
        *self.prepared.lock().unwrap() = Some(Prepared {
            _directory: directory,
            item,
            _version: version.into(),
        });
        Ok(key)
    }
    /// Only an item prepared by this instance can be installed.
    pub fn install(&self, key: &str) -> Result<(), String> {
        let mut guard = self.prepared.lock().unwrap();
        let prepared = guard
            .as_ref()
            .filter(|p| p.item == Path::new(key))
            .ok_or("更新包已失效，请重新下载")?;
        #[cfg(target_os = "macos")]
        macos::install(&self.current, &prepared.item, &prepared._version)?;
        #[cfg(target_os = "windows")]
        windows::install(&self.current, prepared)?;
        let _prepared = guard.take().unwrap();
        #[cfg(target_os = "windows")]
        let _ = _prepared._directory.keep();
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub fn store_update_error_path() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("Windows local application data"))
        .join("Switchboard/update-error.txt")
}
