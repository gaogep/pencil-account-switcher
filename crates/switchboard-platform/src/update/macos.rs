use objc2_foundation::{
    NSBundle, NSFileManager, NSFileManagerItemReplacementOptions, NSString, NSURL,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use switchboard_core::update::compare;

const CONTENT_MISMATCH: &str = "更新包内容与发布版本不符";
const LOCATION_ERROR: &str = "请先把应用拖到「应用程序」文件夹后再更新";
fn url(path: &Path) -> objc2::rc::Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}
pub(super) fn unique_app(directory: &Path) -> Result<PathBuf, String> {
    let apps: Vec<_> = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "app"))
        .collect();
    if apps.len() != 1 {
        return Err("更新包结构异常".into());
    }
    Ok(apps[0].clone())
}
pub(super) fn verify_bundle(app: &Path, version: &str) -> Result<(), String> {
    let bundle = NSBundle::bundleWithURL(&url(app)).ok_or(CONTENT_MISMATCH)?;
    let identifier = bundle.bundleIdentifier().ok_or(CONTENT_MISMATCH)?;
    let actual = bundle
        .objectForInfoDictionaryKey(&NSString::from_str("CFBundleShortVersionString"))
        .and_then(|value| value.downcast::<NSString>().ok())
        .ok_or(CONTENT_MISMATCH)?;
    if identifier.to_string() != "dev.switchboard.Switchboard"
        || !compare(&actual.to_string(), version).is_eq()
    {
        return Err(CONTENT_MISMATCH.into());
    }
    Ok(())
}
pub(super) async fn prepare(
    package: &Path,
    directory: &Path,
    version: &str,
) -> Result<PathBuf, String> {
    let status = tokio::process::Command::new("/usr/bin/ditto")
        .arg("-xk")
        .arg(package)
        .arg(directory)
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("解压更新包失败".into());
    }
    let app = unique_app(directory)?;
    verify_bundle(&app, version)?;
    Ok(app)
}
pub(super) fn assert_replaceable(current: &Path) -> Result<(), String> {
    let resolved = fs::canonicalize(current).map_err(|_| LOCATION_ERROR)?;
    if resolved.to_string_lossy().contains("/AppTranslocation/")
        || current.extension().is_none_or(|ext| ext != "app")
    {
        return Err(LOCATION_ERROR.into());
    }
    let parent = current.parent().ok_or(LOCATION_ERROR)?;
    if !NSFileManager::defaultManager()
        .isWritableFileAtPath(&NSString::from_str(&parent.to_string_lossy()))
    {
        return Err(LOCATION_ERROR.into());
    }
    Ok(())
}
pub(super) fn replace(current: &Path, app: &Path) -> Result<(), String> {
    assert_replaceable(current)?;
    let _ = Command::new("/usr/bin/xattr")
        .arg("-cr")
        .arg(app)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    NSFileManager::defaultManager()
        .replaceItemAtURL_withItemAtURL_backupItemName_options_resultingItemURL_error(
            &url(current),
            &url(app),
            None,
            NSFileManagerItemReplacementOptions::empty(),
            None,
        )
        .map_err(|e| e.localizedDescription().to_string())
}
pub(super) fn install(current: &Path, app: &Path, version: &str) -> Result<(), String> {
    verify_bundle(app, version)?;
    replace(current, app)?;
    // Pass the path as argv; it is never interpolated into shell source.
    Command::new("/bin/sh")
        .args([
            "-c",
            "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.2; done; /usr/bin/open \"$2\"",
            "sh",
        ])
        .arg(std::process::id().to_string())
        .arg(current)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bundle(root: &Path, name: &str, id: &str, version: &str) -> PathBuf {
        let path = root.join(format!("{name}.app"));
        fs::create_dir_all(path.join("Contents")).unwrap();
        fs::write(path.join("Contents/Info.plist"), format!(r#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>{id}</string><key>CFBundleShortVersionString</key><string>{version}</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#)).unwrap();
        path
    }
    #[test]
    fn rejects_empty_multiple_and_mismatched_bundles() {
        let root = tempfile::tempdir().unwrap();
        assert!(unique_app(root.path()).is_err());
        let app = bundle(root.path(), "One", "dev.switchboard.Switchboard", "2.0.0");
        assert_eq!(unique_app(root.path()).unwrap(), app);
        assert!(verify_bundle(&app, "2.0.0").is_ok());
        assert!(verify_bundle(&app, "2.0.1").is_err());
        let other = bundle(root.path(), "Two", "wrong.id", "2.0.0");
        assert!(verify_bundle(&other, "2.0.0").is_err());
        assert!(unique_app(root.path()).is_err());
    }
    #[tokio::test]
    async fn archive_prepare_validates_and_rejects_unknown_install_keys() {
        let root = tempfile::tempdir().unwrap();
        let source = bundle(
            root.path(),
            "Update",
            "dev.switchboard.Switchboard",
            "2.0.0",
        );
        let package = root.path().join("update.zip");
        assert!(Command::new("/usr/bin/ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(&source)
            .arg(&package)
            .status()
            .unwrap()
            .success());
        let installer = super::super::NativeUpdateInstaller::new(root.path().join("Current.app"));
        assert!(installer.install("/unprepared.app").is_err());
        assert!(installer.prepare(&package, "2.0.1").await.is_err());
        let key = installer.prepare(&package, "2.0.0").await.unwrap();
        assert!(Path::new(&key).is_dir());
        assert!(installer.install("/unprepared.app").is_err());
        drop(installer);
        assert!(!Path::new(&key).exists());
    }
    #[test]
    fn refuses_translocation_before_replacing() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("AppTranslocation/X/App.app");
        fs::create_dir_all(&path).unwrap();
        assert_eq!(assert_replaceable(&path).unwrap_err(), LOCATION_ERROR);
    }
    #[tokio::test]
    #[ignore = "requires two signed release archives; run with upgrade-rehearsal.yml"]
    async fn signed_release_replacement() {
        let previous = PathBuf::from(
            std::env::var_os("SWITCHBOARD_PREVIOUS_ARCHIVE").expect("previous signed archive"),
        );
        let next = PathBuf::from(
            std::env::var_os("SWITCHBOARD_NEXT_ARCHIVE").expect("next signed archive"),
        );
        let previous_version = std::env::var("SWITCHBOARD_PREVIOUS_VERSION").unwrap();
        let next_version = std::env::var("SWITCHBOARD_NEXT_VERSION").unwrap();
        for archive in [&previous, &next] {
            let signature = fs::read_to_string(format!("{}.sig", archive.display())).unwrap();
            switchboard_core::update::verify_signature(&fs::read(archive).unwrap(), &signature)
                .unwrap();
        }
        let root = tempfile::tempdir().unwrap();
        let old_stage = root.path().join("old");
        let new_stage = root.path().join("new");
        fs::create_dir_all(&old_stage).unwrap();
        fs::create_dir_all(&new_stage).unwrap();
        let old_app = prepare(&previous, &old_stage, &previous_version)
            .await
            .unwrap();
        let new_app = prepare(&next, &new_stage, &next_version).await.unwrap();
        let current = root.path().join("Installed app.app");
        fs::rename(old_app, &current).unwrap();
        let data = root.path().join("user data");
        fs::create_dir_all(data.join("credentials")).unwrap();
        let protected = [
            ("store.json", include_bytes!("../../../switchboard-core/tests/fixtures/v1.0.1/store.json").as_slice()),
            ("credentials/11111111-1111-4111-8111-111111111111.json", include_bytes!("../../../switchboard-core/tests/fixtures/v1.0.1/credentials/11111111-1111-4111-8111-111111111111.json").as_slice()),
        ];
        for (name, bytes) in protected {
            fs::write(data.join(name), bytes).unwrap();
        }
        replace(&current, &new_app).unwrap();
        verify_bundle(&current, &next_version).unwrap();
        assert!(Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&current)
            .status()
            .unwrap()
            .success());
        for (name, bytes) in protected {
            assert_eq!(fs::read(data.join(name)).unwrap(), bytes);
        }
        println!("Signed macOS release replacement {previous_version} -> {next_version}: bundle and protected fixture bytes passed; UI restart and real startup registration require manual acceptance");
    }
    #[test]
    fn refuses_non_writable_parent_before_replacing() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let app = bundle(
            root.path(),
            "Current",
            "dev.switchboard.Switchboard",
            "1.0.1",
        );
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let result = assert_replaceable(&app);
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(result.unwrap_err(), LOCATION_ERROR);
    }
    #[test]
    fn replacement_uses_new_bundle_and_retains_user_data() {
        let root = tempfile::tempdir().unwrap();
        let old = bundle(
            root.path(),
            "Installed",
            "dev.switchboard.Switchboard",
            "1.0.1",
        );
        let new = bundle(
            root.path(),
            "Staged",
            "dev.switchboard.Switchboard",
            "2.0.0",
        );
        let data = root.path().join("store.json");
        fs::write(&data, b"protected accounts").unwrap();
        replace(&old, &new).unwrap();
        verify_bundle(&old, "2.0.0").unwrap();
        assert_eq!(fs::read(data).unwrap(), b"protected accounts");
    }
}
