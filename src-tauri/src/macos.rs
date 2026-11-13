mod ipc;
mod panel;
mod services;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use switchboard_core::{
    app_model,
    pen_adapter::PenTargetAdapter,
    store::{FileCredentialStore, Store},
};
use switchboard_platform::macos::MacPlatform;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub struct ExitState(AtomicBool);
pub fn quit(app: &tauri::AppHandle) {
    if app.state::<ExitState>().0.swap(true, Ordering::SeqCst) {
        return;
    }
    let model = app.state::<panel::PanelState>().model.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        model.flush().await;
        app.exit(0);
    });
}
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    let services = services::Services::new(app.handle().clone());
    let home = std::path::PathBuf::from(std::env::var_os("HOME").ok_or("missing user home")?);
    let model = tauri::async_runtime::block_on(async {
        app_model::start(
            Arc::new(PenTargetAdapter::new(home, MacPlatform)),
            services,
            Arc::new(FileCredentialStore::new(&MacPlatform)),
            Store::new(&MacPlatform),
            app.package_info().version.to_string(),
            std::env::var("SWITCHBOARD_DEMO").as_deref() == Ok("1"),
        )
    });
    app.manage(panel::PanelState::new(model));
    app.manage(ExitState(AtomicBool::new(false)));
    let open = MenuItem::with_id(app, "open", "打开面板", true, None::<&str>)?;
    let version = MenuItem::with_id(
        app,
        "version",
        format!("Pencil Account Switcher {}", app.package_info().version),
        false,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit = MenuItem::with_id(
        app,
        "exit",
        "退出 Pencil Account Switcher",
        true,
        Some("Cmd+Q"),
    )?;
    let menu = Menu::with_items(app, &[&open, &version, &separator, &exit])?;
    TrayIconBuilder::with_id("switchboard")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray.png"
        ))?)
        .icon_as_template(true)
        .tooltip("Pencil Account Switcher")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => panel::show(app),
            "exit" => quit(app),
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                panel::toggle(tray.app_handle());
            }
        })
        .build(app)?;
    panel::setup(app.handle())?;
    ipc::publish(app.handle());
    if std::env::var("SWITCHBOARD_AUTO_OPEN").as_deref() == Ok("1") {
        panel::show(app.handle());
    }
    Ok(())
}
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![
            ipc::get_state,
            ipc::add_account,
            ipc::cancel_add_account,
            ipc::import_current_login,
            ipc::switch_to,
            ipc::remove_account,
            ipc::refresh_quotas,
            ipc::set_launch_at_login,
            ipc::check_updates,
            ipc::start_update,
            ipc::postpone_update,
            ipc::install_update,
            ipc::open_update_page,
            ipc::hide_panel,
            ipc::set_panel_height,
            ipc::set_confirmation_open,
            ipc::quit_app
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("initialize Switchboard");
    app.run(|app, event| match event {
        tauri::RunEvent::Reopen { .. } => panel::show(app),
        tauri::RunEvent::ExitRequested { api, .. }
            if !app.state::<ExitState>().0.load(Ordering::SeqCst) =>
        {
            api.prevent_exit();
            quit(app);
        }
        _ => (),
    });
}
