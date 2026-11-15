use ts_rs::TS;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("usage: export_bindings <output-directory>")?;
    switchboard_core::ipc::IpcRequest::export_all_to(&output)?;
    switchboard_core::ipc::IpcEvents::export_all_to(&output)?;
    Ok(())
}
