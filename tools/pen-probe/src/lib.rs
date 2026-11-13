#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::run;

#[cfg(not(windows))]
pub fn run(_: &[String]) -> Result<serde_json::Value, String> {
    Err("pen-probe only supports Windows".into())
}
