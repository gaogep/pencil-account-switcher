use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use switchboard_core::app_model::{AppHandle as ModelHandle, Command, UpdateState};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};
use windows_sys::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST},
};
const SHADOW: f64 = 10.0;
pub struct PanelState {
    pub model: ModelHandle,
    pub inner: Mutex<PanelGeometry>,
}
pub struct PanelGeometry {
    shown: Option<Instant>,
    height: f64,
    pub confirmation_open: bool,
}
impl PanelState {
    pub fn new(model: ModelHandle) -> Self {
        Self {
            model,
            inner: Mutex::new(PanelGeometry {
                shown: None,
                height: 400.0,
                confirmation_open: false,
            }),
        }
    }
}
pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let window =
        tauri::WebviewWindowBuilder::new(app, "panel", tauri::WebviewUrl::App("index.html".into()))
            .title("Pencil Account Switcher")
            .inner_size(430.0, 420.0)
            .visible(false)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .initialization_script("document.addEventListener('DOMContentLoaded', () => document.documentElement.classList.add('platform-windows'), { once: true });")
            .build()?;
    let app = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if !app
                .state::<super::ExitState>()
                .0
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                api.prevent_close();
                hide(&app);
            }
        }
        if matches!(event, tauri::WindowEvent::Focused(false)) {
            let shown = app.state::<PanelState>().inner.lock().unwrap().shown;
            if let Some(shown) = shown {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let delay = Duration::from_millis(300).saturating_sub(shown.elapsed());
                    tokio::time::sleep(delay.max(Duration::from_millis(40))).await;
                    let next = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if next.state::<PanelState>().inner.lock().unwrap().shown != Some(shown) {
                            return;
                        }
                        if next
                            .get_webview_window("panel")
                            .is_some_and(|w| w.is_focused().unwrap_or(false))
                        {
                            return;
                        }
                        hide(&next);
                    });
                });
            }
        }
    });
    Ok(())
}
fn keep_open(app: &AppHandle) -> bool {
    let state = app.state::<PanelState>();
    let view = state.model.state();
    view.adding_account
        || view.switching_to.is_some()
        || matches!(
            view.update,
            UpdateState::Downloading { .. } | UpdateState::Ready { .. }
        )
        || state.inner.lock().unwrap().confirmation_open
}
fn position(app: &AppHandle) {
    let Some(window) = app.get_webview_window("panel") else {
        return;
    };
    let anchor = app
        .tray_by_id("switchboard")
        .and_then(|tray| tray.rect().ok().flatten());
    let rect = anchor.map(|rect| {
        (
            rect.position.to_physical::<f64>(1.0),
            rect.size.to_physical::<f64>(1.0),
        )
    });
    let point = rect
        .map(|(p, size)| PhysicalPosition::new(p.x + size.width / 2.0, p.y + size.height / 2.0))
        .or_else(|| app.cursor_position().ok())
        .unwrap_or(PhysicalPosition::new(0.0, 0.0));
    let monitor = app
        .monitor_from_point(point.x, point.y)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe {
        GetMonitorInfoW(
            MonitorFromPoint(
                POINT {
                    x: point.x as i32,
                    y: point.y as i32,
                },
                MONITOR_DEFAULTTONEAREST,
            ),
            &mut info,
        )
    } == 0
    {
        return;
    }
    let work = info.rcWork;
    let full = info.rcMonitor;
    let height = (app.state::<PanelState>().inner.lock().unwrap().height * scale)
        .min((work.bottom - work.top) as f64 - 16.0 * scale)
        .max(1.0);
    let width = 410.0 * scale;
    let gap = 6.0 * scale;
    let (x, y) = if let Some((p, s)) = rect.filter(|(p, s)| {
        s.width > 0.0
            && s.height > 0.0
            && p.x >= full.left as f64
            && p.x < full.right as f64
            && p.y >= full.top as f64
            && p.y < full.bottom as f64
    }) {
        if work.top > full.top && p.y + s.height / 2.0 < work.top as f64 {
            (p.x + s.width / 2.0 - width / 2.0, p.y + s.height + gap)
        } else if work.left > full.left && p.x + s.width / 2.0 < work.left as f64 {
            (p.x + s.width + gap, p.y + s.height / 2.0 - height / 2.0)
        } else if work.right < full.right && p.x + s.width / 2.0 >= work.right as f64 {
            (p.x - width - gap, p.y + s.height / 2.0 - height / 2.0)
        } else {
            (p.x + s.width / 2.0 - width / 2.0, p.y - height - gap)
        }
    } else {
        (
            work.right as f64 - width - 12.0 * scale,
            work.bottom as f64 - height - 12.0 * scale,
        )
    };
    let margin = 8.0 * scale;
    let x = x
        .max(work.left as f64 + margin)
        .min((work.right as f64 - width - margin).max(work.left as f64 + margin));
    let y = y
        .max(work.top as f64 + margin)
        .min((work.bottom as f64 - height - margin).max(work.top as f64 + margin));
    let _ = window.eval(format!(
        "document.documentElement.style.setProperty('--panel-max-height', '{}px')",
        ((work.bottom - work.top) as f64 / scale - 16.0).max(1.0)
    ));
    let _ = window.set_size(PhysicalSize::new(
        (width + SHADOW * 2.0 * scale).round() as u32,
        (height + SHADOW * 2.0 * scale).round() as u32,
    ));
    let _ = window.set_position(PhysicalPosition::new(
        (x - SHADOW * scale).round() as i32,
        (y - SHADOW * scale).round() as i32,
    ));
}
pub fn show(app: &AppHandle) {
    position(app);
    if let Some(window) = app.get_webview_window("panel") {
        app.state::<PanelState>().inner.lock().unwrap().shown = Some(Instant::now());
        app.state::<PanelState>().model.send(Command::SyncIdentity);
        app.state::<PanelState>()
            .model
            .send(Command::CheckUpdates(false));
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("panel-shown", ());
    }
}
pub fn show_if_hidden(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("panel") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.set_focus();
        } else {
            show(app);
        }
    }
}
pub fn hide(app: &AppHandle) {
    if !keep_open(app) {
        hide_for_menu(app);
    }
}
pub fn hide_for_menu(app: &AppHandle) {
    app.state::<PanelState>().inner.lock().unwrap().shown = None;
    if let Some(window) = app.get_webview_window("panel") {
        let _ = window.hide();
    }
}
pub fn toggle(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("panel") {
        if window.is_visible().unwrap_or(false) {
            if app
                .state::<PanelState>()
                .inner
                .lock()
                .unwrap()
                .shown
                .is_some_and(|t| t.elapsed() < Duration::from_millis(350))
            {
                let _ = window.set_focus();
            } else {
                hide(app);
            }
        } else {
            show(app);
        }
    }
}
pub fn resize(app: &AppHandle, height: f64) -> Result<(), String> {
    if !height.is_finite() {
        return Err("面板高度无效".into());
    }
    app.state::<PanelState>().inner.lock().unwrap().height = height.clamp(200.0, 900.0);
    position(app);
    Ok(())
}
