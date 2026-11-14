#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod ipc;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;
fn main() {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    native::run();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    eprintln!("Switchboard supports macOS and Windows");
}
