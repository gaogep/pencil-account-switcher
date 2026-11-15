//! Isolated CI fixtures for TEST_PLAN P-PR-01..05. No real Pen process is closed.
use std::{
    fs,
    mem::zeroed,
    path::{Path, PathBuf},
    process::Command,
    ptr::null,
    time::{Duration, Instant},
};
use switchboard_core::pen_adapter::PenPlatform;
use switchboard_platform::{windows::WindowsPlatform, Platform};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM, LRESULT, RECT, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        Threading::{
            OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
        },
    },
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
            GetWindowRect, PostQuitMessage, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
            GWLP_USERDATA, MSG, WM_CLOSE, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        },
    },
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn fixture_directory() -> PathBuf {
    PathBuf::from(std::env::var_os("SWITCHBOARD_PEN_FIXTURE").unwrap())
}
pub fn is_fixture() -> bool {
    std::env::current_exe()
        .unwrap()
        .file_name()
        .is_some_and(|name| name == "Pen.exe")
}
unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_CLOSE {
        if std::env::var("SWITCHBOARD_PEN_MODE").as_deref() == Ok("ignore") {
            return 0;
        }
        fs::write(
            fixture_directory().join("closed.txt"),
            GetWindowLongPtrW(hwnd, GWLP_USERDATA).to_string(),
        )
        .unwrap();
        PostQuitMessage(0);
        return 0;
    }
    DefWindowProcW(hwnd, message, w, l)
}
pub fn fixture() {
    unsafe {
        let instance = GetModuleHandleW(null());
        let class = wide("Chrome_WidgetWin_1");
        let definition = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        assert_ne!(RegisterClassW(&definition), 0);
        let count = if std::env::var("SWITCHBOARD_PEN_MODE").as_deref() == Ok("multiple") {
            2
        } else {
            1
        };
        let mut bounds = Vec::new();
        for index in 0..count {
            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                wide("pen.dev").as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                80 + index * 400,
                100 + index * 20,
                320,
                240,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                null(),
            );
            assert!(!window.is_null());
            SetWindowLongPtrW(window, GWLP_USERDATA, index as isize);
            let mut rect: RECT = zeroed();
            assert_ne!(GetWindowRect(window, &mut rect), 0);
            let scale = GetDpiForWindow(window) as f64 / 96.0;
            bounds.push(serde_json::json!({"x": rect.left as f64 / scale, "y": rect.top as f64 / scale, "width": (rect.right - rect.left) as f64 / scale, "height": (rect.bottom - rect.top) as f64 / scale}));
        }
        fs::write(
            fixture_directory().join("ready.json"),
            serde_json::to_vec(&serde_json::json!({"pid": std::process::id(), "bounds": bounds}))
                .unwrap(),
        )
        .unwrap();
        let mut message: MSG = zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

struct Environment {
    app_data: Option<std::ffi::OsString>,
}
impl Drop for Environment {
    fn drop(&mut self) {
        if let Some(value) = &self.app_data {
            std::env::set_var("APPDATA", value);
        } else {
            std::env::remove_var("APPDATA");
        }
        std::env::remove_var("SWITCHBOARD_PEN_FIXTURE");
        std::env::remove_var("SWITCHBOARD_PEN_MODE");
    }
}
struct OwnedProcess {
    pid: u32,
    executable: PathBuf,
    child: Option<std::process::Child>,
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
        // Force cleanup is restricted to the private fixture executable and PID.
        // The production platform implementation is still required to use WM_CLOSE.
        unsafe {
            let handle = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                0,
                self.pid,
            );
            if handle.is_null() {
                return;
            }
            let mut buffer = vec![0u16; 32768];
            let mut length = buffer.len() as u32;
            if QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) != 0 {
                let actual = PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]));
                if let (Ok(actual), Ok(expected)) =
                    (fs::canonicalize(actual), fs::canonicalize(&self.executable))
                {
                    if actual == expected {
                        TerminateProcess(handle, 0);
                        WaitForSingleObject(handle, 2000);
                    }
                }
            }
            CloseHandle(handle);
        }
    }
}
fn ready(directory: &Path) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(bytes) = fs::read(directory.join("ready.json")) {
            if let Ok(value) = serde_json::from_slice(&bytes) {
                return value;
            }
        }
        assert!(
            Instant::now() < deadline,
            "fixture window did not become ready"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn start(executable: &Path, directory: &Path, mode: &str) -> (OwnedProcess, serde_json::Value) {
    let _ = fs::remove_file(directory.join("ready.json"));
    let _ = fs::remove_file(directory.join("closed.txt"));
    std::env::set_var("SWITCHBOARD_PEN_MODE", mode);
    let child = Command::new(executable).spawn().unwrap();
    let owned = OwnedProcess {
        pid: child.id(),
        executable: executable.to_owned(),
        child: Some(child),
    };
    let state = ready(directory);
    assert_eq!(state["pid"].as_u64(), Some(owned.pid as u64));
    (owned, state)
}

fn startup_registry() {
    use windows_sys::Win32::System::Registry::RegOverridePredefKey;
    use winreg::{
        enums::{HKEY_CURRENT_USER, REG_BINARY},
        RegKey, RegValue,
    };
    struct PrivateRegistry {
        key: Option<RegKey>,
        path: String,
    }
    impl Drop for PrivateRegistry {
        fn drop(&mut self) {
            unsafe {
                RegOverridePredefKey(HKEY_CURRENT_USER, std::ptr::null_mut());
            }
            drop(self.key.take());
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.path);
        }
    }
    // HKCU is overridden in this single-threaded fixture process only. The real
    // user's Run / StartupApproved values are never changed, even on assertion failure.
    let path = format!(r"Software\SwitchboardCI\Platform-{}", std::process::id());
    let (key, created) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(&path)
        .unwrap();
    assert_eq!(created, winreg::enums::REG_CREATED_NEW_KEY);
    assert_eq!(
        unsafe { RegOverridePredefKey(HKEY_CURRENT_USER, key.raw_handle()) },
        0
    );
    let scope = PrivateRegistry {
        key: Some(key),
        path,
    };
    let platform = WindowsPlatform::default();
    assert!(!platform.launch_at_login().unwrap());
    platform.set_launch_at_login(true).unwrap();
    assert!(platform.launch_at_login().unwrap());
    let (approved, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run")
        .unwrap();
    for disabled in [3, 7] {
        let mut bytes = vec![0; 12];
        bytes[0] = disabled;
        approved
            .set_raw_value(
                "Switchboard",
                &RegValue {
                    bytes: bytes.into(),
                    vtype: REG_BINARY,
                },
            )
            .unwrap();
        assert!(!platform.launch_at_login().unwrap());
        assert!(platform.set_launch_at_login(true).is_err());
    }
    platform.set_launch_at_login(false).unwrap();
    assert!(!platform.launch_at_login().unwrap());
    let run = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .unwrap();
    assert_eq!(
        run.get_raw_value("Switchboard").unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    drop(run);
    drop(approved);
    drop(scope);
    println!("P-AS-02/03: native Run write/read/delete and Windows-disabled status passed in a private HKCU mapping");
}

pub fn run() {
    if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true") {
        println!("Windows process/window fixture cases are reserved for an isolated CI runner");
        return;
    }
    startup_registry();
    let platform = WindowsPlatform::default();
    assert!(
        platform.find_target_processes().is_empty(),
        "an existing Pen process must not be touched by fixtures"
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert!(runtime.block_on(platform.terminate_and_wait(Duration::from_millis(200))));
    let root = tempfile::Builder::new()
        .prefix("Switchboard Pen fixture ")
        .tempdir()
        .unwrap();
    let executable = root.path().join("Pen.exe");
    fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    let _environment = Environment {
        app_data: std::env::var_os("APPDATA"),
    };
    std::env::set_var("APPDATA", root.path());
    std::env::set_var("SWITCHBOARD_PEN_FIXTURE", root.path());
    fs::create_dir_all(root.path().join("Pen")).unwrap();

    // P-PR-02: find, close, remember a portable path, and relaunch via CreateProcessW.
    let (single, _) = start(&executable, root.path(), "single");
    assert!(platform.find_target_processes().contains(&single.pid));
    assert!(platform.can_launch());
    assert!(runtime.block_on(platform.terminate_and_wait(Duration::from_secs(3))));
    assert_eq!(
        fs::read_to_string(root.path().join("closed.txt")).unwrap(),
        "0"
    );
    assert_eq!(
        fs::canonicalize(platform.resolve_target_app().unwrap()).unwrap(),
        fs::canonicalize(&executable).unwrap()
    );
    drop(single);
    fs::remove_file(root.path().join("ready.json")).unwrap();
    runtime.block_on(platform.launch()).unwrap();
    let relaunched = ready(root.path());
    let relaunched = OwnedProcess {
        pid: relaunched["pid"].as_u64().unwrap() as u32,
        executable: executable.clone(),
        child: None,
    };
    assert!(platform.find_target_processes().contains(&relaunched.pid));
    assert!(runtime.block_on(platform.terminate_and_wait(Duration::from_secs(3))));
    drop(relaunched);

    // P-PR-03: a refusal to close times out without terminating the process.
    let (ignored, _) = start(&executable, root.path(), "ignore");
    assert!(!runtime.block_on(platform.terminate_and_wait(Duration::from_millis(250))));
    assert!(platform.find_target_processes().contains(&ignored.pid));
    drop(ignored);

    // P-PR-04: two identical titles; select only the configured window.
    let (multiple, state) = start(&executable, root.path(), "multiple");
    fs::write(
        root.path().join("Pen/config.json"),
        serde_json::to_vec(&serde_json::json!({"windowBounds": state["bounds"][1]})).unwrap(),
    )
    .unwrap();
    assert!(runtime.block_on(platform.terminate_and_wait(Duration::from_secs(3))));
    assert_eq!(
        fs::read_to_string(root.path().join("closed.txt")).unwrap(),
        "1"
    );
    drop(multiple);

    // P-PR-05: no unique match must not send a close message to either window.
    let (ambiguous, _) = start(&executable, root.path(), "multiple");
    fs::write(
        root.path().join("Pen/config.json"),
        br#"{"windowBounds":{"x":-5000,"y":-5000,"width":10,"height":10}}"#,
    )
    .unwrap();
    assert!(!runtime.block_on(platform.terminate_and_wait(Duration::from_millis(250))));
    assert!(platform.find_target_processes().contains(&ambiguous.pid));
    assert!(!root.path().join("closed.txt").exists());
    drop(ambiguous);
    assert!(platform.find_target_processes().is_empty());
    println!("P-PR-01..05: native discovery, portable relaunch, graceful close, refusal timeout and unique-window selection passed");
}
