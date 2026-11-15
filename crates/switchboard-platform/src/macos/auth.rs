use super::on_main;
use block2::RcBlock;
use objc2::{
    define_class, msg_send, rc::Retained, runtime::ProtocolObject, AnyThread, DefinedClass,
    MainThreadOnly,
};
use objc2_app_kit::{NSApplication, NSWindow};
use objc2_authentication_services::{
    ASPresentationAnchor, ASWebAuthenticationPresentationContextProviding,
    ASWebAuthenticationSession,
};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol, NSString, NSURL};
use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
use switchboard_core::web_login::AuthWindow;
use tokio::sync::watch;

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Retained<NSWindow>]
    struct AnchorProvider;
    unsafe impl NSObjectProtocol for AnchorProvider {}
    unsafe impl ASWebAuthenticationPresentationContextProviding for AnchorProvider {
        #[unsafe(method_id(presentationAnchorForWebAuthenticationSession:))]
        fn anchor(&self, _: &ASWebAuthenticationSession) -> Retained<ASPresentationAnchor> { self.ivars().clone().into_super().into_super() }
    }
);
struct Session {
    auth: Retained<ASWebAuthenticationSession>,
    _provider: Retained<AnchorProvider>,
}
thread_local! { static SESSIONS: RefCell<HashMap<u64, Session>> = RefCell::new(HashMap::new()); }
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub struct MacAuthWindow {
    id: u64,
    closed: Mutex<watch::Receiver<Option<Result<(), String>>>>,
}
impl Default for MacAuthWindow {
    fn default() -> Self {
        let (_, closed) = watch::channel(None);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            closed: Mutex::new(closed),
        }
    }
}
impl AuthWindow for MacAuthWindow {
    fn open(&self, url: &str) -> Result<(), String> {
        self.cancel();
        let (tx, rx) = watch::channel(None);
        *self.closed.lock().unwrap_or_else(|e| e.into_inner()) = rx;
        let id = self.id;
        let url = url.to_string();
        on_main(move |mtm| unsafe {
            let url = NSURL::URLWithString(&NSString::from_str(&url))
                .ok_or("登录接口返回了无效的网址")?;
            let app = NSApplication::sharedApplication(mtm);
            let window = app
                .keyWindow()
                .or_else(|| app.mainWindow())
                .unwrap_or_else(|| NSWindow::new(mtm));
            let provider: Retained<AnchorProvider> =
                msg_send![super(AnchorProvider::alloc(mtm).set_ivars(window)), init];
            let completion = RcBlock::new(move |_: *mut NSURL, error: *mut NSError| {
                if let Some(error) = error.as_ref() {
                    let result = if error.code() == 1 {
                        Ok(())
                    } else {
                        Err(error.localizedDescription().to_string())
                    };
                    tx.send_replace(Some(result));
                }
            });
            // The legacy initializer is required for the macOS 13 deployment target.
            #[allow(deprecated)]
            let auth = ASWebAuthenticationSession::initWithURL_callbackURLScheme_completionHandler(
                ASWebAuthenticationSession::alloc(),
                &url,
                None,
                RcBlock::as_ptr(&completion),
            );
            auth.setPresentationContextProvider(Some(ProtocolObject::from_ref(&*provider)));
            auth.setPrefersEphemeralWebBrowserSession(true);
            if !auth.start() {
                return Err("无法打开系统认证窗口".into());
            }
            SESSIONS.with(|sessions| {
                sessions.borrow_mut().insert(
                    id,
                    Session {
                        auth,
                        _provider: provider,
                    },
                )
            });
            Ok(())
        })
    }
    fn closed(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        let mut rx = self
            .closed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        Box::pin(async move {
            loop {
                if let Some(result) = rx.borrow_and_update().clone() {
                    return result;
                }
                rx.changed()
                    .await
                    .map_err(|_| "网页登录已取消或超时，原账号保持不变".to_string())?;
            }
        })
    }
    fn cancel(&self) {
        let id = self.id;
        on_main(move |_| {
            let session = SESSIONS.with(|sessions| sessions.borrow_mut().remove(&id));
            if let Some(session) = session {
                unsafe {
                    session.auth.cancel();
                }
            }
        });
    }
}
impl Drop for MacAuthWindow {
    fn drop(&mut self) {
        self.cancel();
    }
}
