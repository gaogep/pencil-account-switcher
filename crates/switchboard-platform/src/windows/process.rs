//! Pen process/window discovery follows the validated pen-probe rules.
use serde_json::Value;
use std::{
    ffi::OsStr,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::null,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, HWND, INVALID_HANDLE_VALUE, LPARAM,
        RECT, STILL_ACTIVE,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            CreateProcessW, GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW,
            DETACHED_PROCESS, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, STARTUPINFOW,
        },
    },
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
            PostMessageW, WM_CLOSE,
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

pub struct Process {
    pub pid: u32,
    pub path: String,
}
struct Window {
    hwnd: isize,
    pid: u32,
    title: String,
    visible: bool,
    rect: [i32; 4],
    dpi: u32,
}

pub fn processes() -> Result<Vec<Process>, String> {
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
                    let code = GetLastError();
                    if code == ERROR_INVALID_PARAMETER {
                        ok = Process32NextW(snapshot, &mut entry);
                        continue;
                    }
                    CloseHandle(snapshot);
                    return Err(std::io::Error::from_raw_os_error(code as i32).to_string());
                }
                let mut path = vec![0u16; 32768];
                let mut len = path.len() as u32;
                let status = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
                if status == 0 {
                    let code = GetLastError();
                    let mut exit_code = 0;
                    let exited = GetExitCodeProcess(process, &mut exit_code) != 0
                        && exit_code != STILL_ACTIVE as u32;
                    CloseHandle(process);
                    if exited {
                        ok = Process32NextW(snapshot, &mut entry);
                        continue;
                    }
                    CloseHandle(snapshot);
                    return Err(std::io::Error::from_raw_os_error(code as i32).to_string());
                }
                CloseHandle(process);
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
    let mut rect: RECT = zeroed();
    GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
    GetWindowRect(hwnd, &mut rect);
    windows.push(Window {
        hwnd: hwnd as isize,
        pid,
        title: string(&title),
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

pub fn request_quit(running: &[Process]) -> Result<(), String> {
    if running.is_empty() {
        return Ok(());
    }
    let window = main_window(running)?;
    if unsafe { PostMessageW(window.hwnd as HWND, WM_CLOSE, 0, 0) } == 0 {
        return Err(error());
    }
    Ok(())
}
pub fn launch(path: &Path) -> Result<(), String> {
    unsafe {
        let path = path.to_str().ok_or("Pen 路径不是有效的 Unicode")?;
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
        Ok(())
    }
}
