use objc2_app_kit::NSScreen;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use switchboard_core::app_model::{AppHandle as ModelHandle, Command, UpdateState};
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask,
};

tauri_panel! { panel!(SwitchboardPanel { config: { can_become_key_window: true, can_become_main_window: false, is_floating_panel: true, hides_on_deactivate: false } }) }

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
    PanelBuilder::<_, SwitchboardPanel>::new(app, "panel")
        .url(tauri::WebviewUrl::App("index.html".into()))
        .level(PanelLevel::Status)
        .style_mask(StyleMask::empty().nonactivating_panel())
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary(),
        )
        .size(tauri::Size::Logical(tauri::LogicalSize::new(410.0, 400.0)))
        .transparent(true)
        .with_window(|window| {
            window
                .visible(false)
                .decorations(false)
                .resizable(false)
                .skip_taskbar(true)
                .min_inner_size(410.0, 200.0)
                .max_inner_size(410.0, 900.0)
        })
        .build()?;
    let panel = app
        .get_webview_panel("panel")
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    panel.set_has_shadow(true);
    panel.set_opaque(false);
    // `to_window()` converts the NSPanel back into an NSWindow and unregisters
    // it. Query Tauri's existing handle instead, keeping both registries and
    // the original native delegate intact.
    let window = app
        .get_webview_window("panel")
        .ok_or_else(|| std::io::Error::other("panel webview window is missing"))?;
    let app = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Focused(false)) {
            // Defer until the tray action (if any) has been delivered.
            let shown = app.state::<PanelState>().inner.lock().unwrap().shown;
            if let Some(shown) = shown {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let remaining = Duration::from_millis(300).saturating_sub(shown.elapsed());
                    tokio::time::sleep(remaining.max(Duration::from_millis(40))).await;
                    let next = app.clone();
                    let _ = app.run_on_main_thread(move || blur(&next, shown));
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
fn anchor(app: &AppHandle) -> Option<(Option<NSRect>, NSRect)> {
    app.tray_by_id("switchboard")?
        .with_inner_tray_icon(|tray| {
            let mtm = MainThreadMarker::new().expect("tray runs on main thread");
            let window = tray
                .ns_status_item()
                .and_then(|item| item.button(mtm))
                .and_then(|button| button.window());
            let screen = window
                .as_ref()
                .and_then(|w| w.screen())
                .or_else(|| NSScreen::mainScreen(mtm));
            screen.map(|screen| (window.map(|w| w.frame()), screen.visibleFrame()))
        })
        .ok()
        .flatten()
}
fn position(app: &AppHandle) {
    let mtm = MainThreadMarker::new().expect("panel runs on main thread");
    let Some((anchor, screen)) =
        anchor(app).or_else(|| NSScreen::mainScreen(mtm).map(|s| (None, s.visibleFrame())))
    else {
        return;
    };
    let height = app
        .state::<PanelState>()
        .inner
        .lock()
        .unwrap()
        .height
        .min((screen.size.height - 16.0).max(1.0));
    let visible_anchor = anchor.filter(|rect| {
        NSScreen::screens(mtm)
            .iter()
            .any(|s| intersects(s.frame(), *rect))
    });
    let (x, y) = if let Some(rect) = visible_anchor {
        (
            rect.origin.x + rect.size.width / 2.0 - 205.0,
            rect.origin.y - height - 6.0,
        )
    } else {
        (
            screen.origin.x + screen.size.width - 410.0 - 12.0,
            screen.origin.y + screen.size.height - height - 12.0,
        )
    };
    let x = x
        .max(screen.origin.x + 8.0)
        .min((screen.origin.x + screen.size.width - 418.0).max(screen.origin.x + 8.0));
    let y = y
        .max(screen.origin.y + 8.0)
        .min(screen.origin.y + screen.size.height - height - 8.0);
    if let Ok(panel) = app.get_webview_panel("panel") {
        panel.as_panel().setFrame_display(
            NSRect::new(NSPoint::new(x, y), NSSize::new(410.0, height)),
            true,
        );
    }
}
fn intersects(a: NSRect, b: NSRect) -> bool {
    a.origin.x < b.origin.x + b.size.width
        && b.origin.x < a.origin.x + a.size.width
        && a.origin.y < b.origin.y + b.size.height
        && b.origin.y < a.origin.y + a.size.height
}
pub fn show(app: &AppHandle) {
    position(app);
    if let Ok(panel) = app.get_webview_panel("panel") {
        app.state::<PanelState>().inner.lock().unwrap().shown = Some(Instant::now());
        let model = &app.state::<PanelState>().model;
        model.send(Command::SyncIdentity);
        model.send(Command::CheckUpdates(false));
        panel.show_and_make_key();
        let _ = app.emit("panel-shown", ());
    }
}
pub fn hide(app: &AppHandle) {
    if keep_open(app) {
        return;
    }
    hide_for_menu(app);
}
/// An explicit tray menu action can dismiss the panel even during an operation.
pub fn hide_for_menu(app: &AppHandle) {
    app.state::<PanelState>().inner.lock().unwrap().shown = None;
    if let Ok(panel) = app.get_webview_panel("panel") {
        panel.hide();
    }
}
pub fn show_if_hidden(app: &AppHandle) {
    if let Ok(panel) = app.get_webview_panel("panel") {
        if !panel.is_visible() {
            show(app);
        }
    }
}
pub fn toggle(app: &AppHandle) {
    if let Ok(panel) = app.get_webview_panel("panel") {
        if panel.is_visible() {
            if app
                .state::<PanelState>()
                .inner
                .lock()
                .unwrap()
                .shown
                .is_some_and(|t| t.elapsed() < Duration::from_millis(350))
            {
                panel.show_and_make_key();
                return;
            }
            hide(app);
        } else {
            show(app);
        }
    }
}
fn blur(app: &AppHandle, shown: Instant) {
    // Ignore callbacks from a previous show/hide cycle. Pointer location is
    // irrelevant for non-click focus changes such as Cmd+Tab.
    if app.state::<PanelState>().inner.lock().unwrap().shown != Some(shown) {
        return;
    }
    if let Ok(panel) = app.get_webview_panel("panel") {
        if !panel.is_visible() || panel.as_panel().isKeyWindow() {
            return;
        }
    }
    hide(app);
}
pub fn resize(app: &AppHandle, height: f64) -> Result<(), String> {
    if !height.is_finite() {
        return Err("面板高度无效".into());
    }
    app.state::<PanelState>().inner.lock().unwrap().height = height.clamp(200.0, 900.0);
    position(app);
    Ok(())
}
