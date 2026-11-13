#[cfg(target_os = "macos")]
mod macos;
fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
    #[cfg(not(target_os = "macos"))]
    eprintln!("Windows shell is scheduled for P4");
}
