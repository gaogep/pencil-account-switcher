//! Runs on the process main thread so Cocoa / ServiceManagement never deadlocks
//! waiting for a test harness thread's main queue. No real account data is written.
use std::{fs, path::PathBuf};
use switchboard_core::store::{atomic_write, DataDirectory, FileCredentialStore};
use switchboard_platform::Platform;

#[cfg(windows)]
#[path = "support/windows.rs"]
mod windows_fixtures;

fn main() {
    #[cfg(windows)]
    if windows_fixtures::is_fixture() {
        windows_fixtures::fixture();
        return;
    }
    #[cfg(target_os = "macos")]
    let platform = switchboard_platform::macos::MacPlatform;
    #[cfg(target_os = "windows")]
    let platform = switchboard_platform::windows::WindowsPlatform::default();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    check(&platform);
    #[cfg(windows)]
    windows_fixtures::run();
}
fn check(platform: &impl Platform) {
    assert!(platform.data_dir().ends_with("Switchboard"));
    assert!(matches!(platform.os_tag(), "darwin" | "win32"));
    let _ = platform.find_target_processes(); // Read-only process API smoke check.
    let _ = platform.resolve_target_app();
    platform
        .launch_at_login()
        .expect("read native startup status");
    let temp = tempfile::tempdir().unwrap();
    struct Directory(PathBuf);
    impl DataDirectory for Directory {
        fn data_dir(&self) -> PathBuf {
            self.0.clone()
        }
    }
    let store = FileCredentialStore::new(&Directory(temp.path().to_owned()));
    let id = "11111111-1111-4111-8111-111111111111";
    store.save(id, b"old fixture").unwrap();
    store.save(id, b"new fixture").unwrap();
    assert_eq!(store.load(id).unwrap(), b"new fixture");
    let file = temp.path().join("credentials").join(format!("{id}.json"));
    platform.restrict_permissions(&file, false).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(file.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    #[cfg(target_os = "macos")]
    {
        // A locked destination fails after the temporary file has been written.
        // Only this temporary fixture is made immutable, and Drop restores it.
        struct Immutable(PathBuf);
        impl Drop for Immutable {
            fn drop(&mut self) {
                let _ = std::process::Command::new("/usr/bin/chflags")
                    .arg("nouchg")
                    .arg(&self.0)
                    .status();
            }
        }
        let guard = Immutable(file.clone());
        assert!(std::process::Command::new("/usr/bin/chflags")
            .arg("uchg")
            .arg(&file)
            .status()
            .unwrap()
            .success());
        assert!(atomic_write(&file, b"must not replace immutable file", true).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"new fixture");
        assert_eq!(fs::read_dir(file.parent().unwrap()).unwrap().count(), 1);
        drop(guard);
    }
    #[cfg(windows)]
    {
        use std::{
            fs::OpenOptions,
            os::windows::fs::OpenOptionsExt,
            time::{Duration, Instant},
        };
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        let start = Instant::now();
        assert!(atomic_write(&file, b"must not replace locked file", true).is_err());
        assert!(start.elapsed() >= Duration::from_millis(900));
        drop(lock);
        assert_eq!(fs::read(&file).unwrap(), b"new fixture");
        // A transient lock must recover during the retry window.
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        let unlock = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            drop(lock);
        });
        atomic_write(&file, b"after lock", true).unwrap();
        unlock.join().unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"after lock");
    }
    atomic_write(&temp.path().join("state.json"), b"fixture", false).unwrap();
    store.delete(id).unwrap();
    assert!(!file.exists());
    println!("platform-it: native paths, processes, startup read, credential I/O and OS file semantics passed");
}
