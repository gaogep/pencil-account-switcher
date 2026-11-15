use std::{
    future::Future,
    pin::Pin,
    sync::{mpsc, Mutex},
};
use switchboard_core::web_login::AuthWindow;
use tauri::{AppHandle, Manager};
use tokio::sync::watch;
struct Session {
    label: String,
    closed: watch::Sender<bool>,
}
pub struct WindowsAuthWindow {
    app: AppHandle,
    active: Mutex<Option<Session>>,
}
impl WindowsAuthWindow {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            active: Mutex::new(None),
        }
    }
}
impl AuthWindow for WindowsAuthWindow {
    fn open(&self, url: &str) -> Result<(), String> {
        self.cancel();
        let url = url.parse().map_err(|_| "登录接口返回了无效的网址")?;
        let label = format!("login-{}", uuid::Uuid::new_v4());
        let (closed, _) = watch::channel(false);
        *self.active.lock().unwrap() = Some(Session {
            label: label.clone(),
            closed: closed.clone(),
        });
        let app = self.app.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.app
            .run_on_main_thread(move || {
                // Cancellation may arrive while creation is queued on the UI thread.
                if *closed.borrow() {
                    let _ = tx.send(Ok(()));
                    return;
                }
                let result = tauri::WebviewWindowBuilder::new(
                    &app,
                    &label,
                    tauri::WebviewUrl::External(url),
                )
                .title("添加账号 — Pencil Account Switcher")
                .inner_size(720.0, 800.0)
                .incognito(true)
                .build()
                .map_err(|e| e.to_string());
                match result {
                    Ok(window) => {
                        let sender = closed.clone();
                        window.on_window_event(move |event| {
                            if matches!(
                                event,
                                tauri::WindowEvent::CloseRequested { .. }
                                    | tauri::WindowEvent::Destroyed
                            ) {
                                sender.send_replace(true);
                            }
                        });
                        if *closed.borrow() {
                            let _ = window.close();
                        }
                        let _ = tx.send(Ok(()));
                    }
                    Err(error) => {
                        closed.send_replace(true);
                        let _ = tx.send(Err(error));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
    fn closed(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        let receiver = self
            .active
            .lock()
            .unwrap()
            .as_ref()
            .map(|session| session.closed.subscribe());
        Box::pin(async move {
            if let Some(mut receiver) = receiver {
                while !*receiver.borrow_and_update() {
                    if receiver.changed().await.is_err() {
                        break;
                    }
                }
            }
            Ok(())
        })
    }
    fn cancel(&self) {
        if let Some(session) = self.active.lock().unwrap().take() {
            session.closed.send_replace(true);
            let app = self.app.clone();
            let _ = self.app.run_on_main_thread(move || {
                if let Some(window) = app.get_webview_window(&session.label) {
                    let _ = window.close();
                }
            });
        }
    }
}
