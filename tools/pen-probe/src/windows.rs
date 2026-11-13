use serde::Serialize;
use serde_json::{json, Value};
use std::{
    ffi::OsStr,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::{null, null_mut},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, INVALID_HANDLE_VALUE, LPARAM, RECT},
    Networking::WinSock::{GetHostNameW, WSACleanup, WSAStartup, WSADATA},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Registry::*,
        Threading::{
            CreateProcessW, OpenProcess, QueryFullProcessImageNameW, DETACHED_PROCESS,
            PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, STARTUPINFOW,
        },
    },
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
            IsWindowVisible, PostMessageW, WM_CLOSE,
        },
    },
};

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
fn string(buf: &[u16]) -> String {
    String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())])
}
fn error() -> String {
    std::io::Error::last_os_error().to_string()
}

#[derive(Serialize)]
struct Process {
    pid: u32,
    path: String,
}
#[derive(Serialize)]
struct Window {
    hwnd: isize,
    pid: u32,
    title: String,
    class: String,
    visible: bool,
    rect: [i32; 4],
    dpi: u32,
}

fn hostname() -> Result<Value, String> {
    unsafe {
        let mut data: WSADATA = zeroed();
        if WSAStartup(0x0202, &mut data) != 0 {
            return Err("WSAStartup failed".into());
        }
        let mut buffer = [0u16; 256];
        let status = GetHostNameW(buffer.as_mut_ptr(), buffer.len() as i32);
        WSACleanup();
        if status != 0 {
            return Err("GetHostNameW failed".into());
        }
        let name = string(&buffer);
        Ok(
            json!({"hostname": name, "deviceId": format!("{:x}", md5::compute(format!("{}win32x64", name))) }),
        )
    }
}

fn processes() -> Result<Vec<Process>, String> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(error());
        }
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut result = Vec::new();
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            if string(&entry.szExeFile).eq_ignore_ascii_case("Pen.exe") {
                let process =
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if process.is_null() {
                    CloseHandle(snapshot);
                    return Err(error());
                }
                let mut path = vec![0u16; 32768];
                let mut len = path.len() as u32;
                let status = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
                CloseHandle(process);
                if status == 0 {
                    CloseHandle(snapshot);
                    return Err(error());
                }
                result.push(Process {
                    pid: entry.th32ProcessID,
                    path: String::from_utf16_lossy(&path[..len as usize]),
                });
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        Ok(result)
    }
}

unsafe extern "system" fn collect_window(hwnd: HWND, param: LPARAM) -> i32 {
    let windows = &mut *(param as *mut Vec<Window>);
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let mut title = [0u16; 1024];
    let mut class = [0u16; 256];
    let mut rect: RECT = zeroed();
    GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
    GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
    GetWindowRect(hwnd, &mut rect);
    windows.push(Window {
        hwnd: hwnd as isize,
        pid,
        title: string(&title),
        class: string(&class),
        visible: IsWindowVisible(hwnd) != 0,
        rect: [rect.left, rect.top, rect.right, rect.bottom],
        dpi: GetDpiForWindow(hwnd),
    });
    1
}

fn windows(processes: &[Process]) -> Result<Vec<Window>, String> {
    let mut result: Vec<Window> = Vec::new();
    unsafe {
        if EnumWindows(Some(collect_window), &mut result as *mut _ as LPARAM) == 0 {
            return Err(error());
        }
    }
    result.retain(|window| processes.iter().any(|p| p.pid == window.pid));
    Ok(result)
}

fn main_window(processes: &[Process]) -> Result<Window, String> {
    let mut candidates: Vec<_> = windows(processes)?
        .into_iter()
        .filter(|w| w.visible && w.title == "pen.dev")
        .collect();
    if candidates.len() == 1 {
        return Ok(candidates.remove(0));
    }
    if candidates.len() > 1 {
        let config = std::env::var_os("APPDATA").ok_or("无法确定主窗口")?;
        let data = std::fs::read(Path::new(&config).join("Pen/config.json"))
            .map_err(|_| "无法确定主窗口")?;
        let config: Value = serde_json::from_slice(&data).map_err(|_| "无法确定主窗口")?;
        let bounds = &config["windowBounds"];
        let fields: Option<Vec<f64>> = ["x", "y", "width", "height"]
            .iter()
            .map(|key| bounds[*key].as_f64())
            .collect();
        let b = fields.ok_or("无法确定主窗口")?;
        candidates.retain(|w| {
            let scale = w.dpi as f64 / 96.0;
            let expected = [
                b[0] * scale,
                b[1] * scale,
                (b[0] + b[2]) * scale,
                (b[1] + b[3]) * scale,
            ];
            w.rect
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (*actual as f64 - expected).abs() <= 2.0)
        });
        if candidates.len() == 1 {
            return Ok(candidates.remove(0));
        }
    }
    Err("无法确定主窗口".into())
}

fn quit(timeout: u64) -> Result<Value, String> {
    let running = processes()?;
    if running.is_empty() {
        return Ok(json!({"exited": true}));
    }
    let window = main_window(&running)?;
    if unsafe { PostMessageW(window.hwnd as HWND, WM_CLOSE, 0, 0) } == 0 {
        return Err(error());
    }
    let deadline = Instant::now() + Duration::from_secs(timeout);
    loop {
        if processes()?.is_empty() {
            return Ok(json!({"exited": true}));
        }
        if Instant::now() >= deadline {
            return Err("请先退出 Pen".into());
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn launch(path: &str) -> Result<Value, String> {
    unsafe {
        let application = wide(path);
        let mut command = wide(&format!("\"{}\"", path));
        let mut startup: STARTUPINFOW = zeroed();
        startup.cb = size_of::<STARTUPINFOW>() as u32;
        let mut info: PROCESS_INFORMATION = zeroed();
        if CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            DETACHED_PROCESS,
            null(),
            null(),
            &startup,
            &mut info,
        ) == 0
        {
            return Err(error());
        }
        CloseHandle(info.hThread);
        CloseHandle(info.hProcess);
        Ok(json!({"pid": info.dwProcessId}))
    }
}

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

fn registry_read(key: &str, name: &str) -> Result<Option<(u32, Vec<u8>)>, String> {
    unsafe {
        let key = wide(key);
        let name = wide(name);
        let mut kind = 0;
        let mut size = 0;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_ANY,
            &mut kind,
            null_mut(),
            &mut size,
        );
        if status == 2 {
            return Ok(None);
        }
        if status != 0 {
            return Err(format!("registry error {status}"));
        }
        let mut data = vec![0u8; size as usize];
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_ANY,
            &mut kind,
            data.as_mut_ptr().cast(),
            &mut size,
        );
        if status != 0 {
            return Err(format!("registry error {status}"));
        }
        data.truncate(size as usize);
        Ok(Some((kind, data)))
    }
}

fn autostart(action: &str, name: &str, value: Option<&str>) -> Result<Value, String> {
    if action == "set" && value.is_none() {
        return Err("autostart set requires --value".into());
    }
    if action == "get" {
        let raw = registry_read(RUN, name)?;
        let approved = registry_read(APPROVED, name)?;
        let enabled = raw.is_some()
            && approved.as_ref().map_or(true, |(_, data)| {
                data.first().map_or(true, |b| *b != 3 && *b != 7)
            });
        let value = raw.map(|(_, data)| {
            string(
                &data
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
        });
        return Ok(
            json!({"value": value, "enabled": enabled, "startupApproved": approved.map(|(_, data)| data)}),
        );
    }
    unsafe {
        let mut key = null_mut();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(RUN).as_ptr(),
            0,
            null(),
            0,
            KEY_SET_VALUE,
            null(),
            &mut key,
            null_mut(),
        );
        if status != 0 {
            return Err(format!("registry error {status}"));
        }
        let name = wide(name);
        let status = if action == "set" {
            let value = wide(value.ok_or("autostart set requires --value")?);
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        } else if action == "clear" {
            RegDeleteValueW(key, name.as_ptr())
        } else {
            RegCloseKey(key);
            return Err("unknown autostart action".into());
        };
        RegCloseKey(key);
        if status != 0 && !(action == "clear" && status == 2) {
            return Err(format!("registry error {status}"));
        }
        Ok(json!({"action": action, "success": true}))
    }
}

pub fn run(args: &[String]) -> Result<Value, String> {
    match args.first().map(String::as_str) {
        Some("hostname") if args.len() == 1 => hostname(),
        Some("processes") if args.len() == 1 => Ok(json!(processes()?)),
        Some("windows") if args.len() == 1 => Ok(json!(windows(&processes()?)?)),
        Some("main-window") if args.len() == 1 => Ok(json!(main_window(&processes()?)?)),
        Some("quit") if args.len() == 3 && args[1] == "--timeout" => quit(args[2].parse().map_err(|_| "invalid timeout")?),
        Some("launch") if args.len() == 2 => launch(&args[1]),
        Some("autostart") if args.len() >= 4 && args[2] == "--name" => {
            let value = if args.len() == 6 && args[4] == "--value" { Some(args[5].as_str()) } else if args.len() == 4 { None } else { return Err("invalid autostart arguments".into()); };
            autostart(&args[1], &args[3], value)
        }
        _ => Err("usage: hostname | processes | windows | main-window | quit --timeout SECONDS | launch PATH | autostart get|set|clear --name NAME [--value VALUE]".into()),
    }
}
