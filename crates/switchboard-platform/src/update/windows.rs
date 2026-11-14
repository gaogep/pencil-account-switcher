use super::Prepared;
use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
pub(super) async fn prepare(package: &Path, directory: &Path) -> Result<PathBuf, String> {
    let item = directory.join("setup.exe");
    tokio::fs::copy(package, &item)
        .await
        .map_err(|e| e.to_string())?;
    Ok(item)
}
pub(super) fn install(current: &Path, prepared: &Prepared) -> Result<(), String> {
    let script = prepared._directory.path().join("apply-update.ps1");
    fs::write(&script, include_str!("apply-update.ps1")).map_err(|e| e.to_string())?;
    let log = super::store_update_error_path();
    fs::create_dir_all(log.parent().unwrap()).map_err(|e| e.to_string())?;
    let powershell =
        PathBuf::from(std::env::var_os("SystemRoot").ok_or("找不到 Windows 系统目录")?)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-ParentProcessId")
        .arg(std::process::id().to_string())
        .arg("-SetupPath")
        .arg(&prepared.item)
        .arg("-TargetPath")
        .arg(current)
        .arg("-LogPath")
        .arg(log)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn staging_copies_package_and_rejects_unprepared_paths() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("download.exe");
        std::fs::write(&package, b"verified fixture").unwrap();
        let installer = super::super::NativeUpdateInstaller::new(root.path().join("Installed.exe"));
        assert!(installer.install(package.to_str().unwrap()).is_err());
        let key = installer.prepare(&package, "2.0.0").await.unwrap();
        std::fs::write(package, b"download changed").unwrap();
        assert_eq!(std::fs::read(&key).unwrap(), b"verified fixture");
        drop(installer);
        assert!(!std::path::Path::new(&key).exists());
    }
}
