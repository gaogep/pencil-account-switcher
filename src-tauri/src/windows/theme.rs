use tauri::{image::Image, AppHandle, Manager};
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ},
    RegKey,
};
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
fn light() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(PERSONALIZE)
        .and_then(|key| key.get_value::<u32, _>("SystemUsesLightTheme"))
        .unwrap_or(1)
        != 0
}
pub fn icon() -> Result<Image<'static>, tauri::Error> {
    Image::from_bytes(if light() {
        include_bytes!("../../icons/tray-light.ico")
    } else {
        include_bytes!("../../icons/tray-dark.ico")
    })
}
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        use windows_sys::Win32::System::Registry::{
            RegNotifyChangeKeyValue, REG_NOTIFY_CHANGE_LAST_SET,
        };
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(PERSONALIZE, KEY_READ | KEY_NOTIFY)
        else {
            return;
        };
        loop {
            if unsafe {
                RegNotifyChangeKeyValue(
                    key.raw_handle().cast(),
                    0,
                    REG_NOTIFY_CHANGE_LAST_SET,
                    std::ptr::null_mut(),
                    0,
                )
            } != 0
            {
                break;
            }
            if app
                .state::<super::ExitState>()
                .0
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                break;
            }
            if let (Some(tray), Ok(icon)) = (app.tray_by_id("switchboard"), icon()) {
                let _ = tray.set_icon(Some(icon));
            }
        }
    });
}
