//! Native services; all Cocoa objects stay on the macOS main thread.
use std::{
    io,
    path::{Path, PathBuf},
};
use switchboard_core::{store::DataDirectory, target::TargetFuture};

pub trait Platform: DataDirectory + Send + Sync {
    fn os_tag(&self) -> &'static str;
    fn find_target_processes(&self) -> Vec<u32>;
    fn request_quit(&self, pids: &[u32]);
    fn resolve_target_app(&self) -> Option<PathBuf>;
    fn launch_target<'a>(&'a self, app: &'a Path) -> TargetFuture<'a, io::Result<()>>;
    fn launch_at_login(&self) -> io::Result<bool>;
    fn set_launch_at_login(&self, on: bool) -> io::Result<()>;
    fn restrict_permissions(&self, path: &Path, dir: bool) -> io::Result<()>;
}

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;
