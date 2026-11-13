use crate::target::TargetSnapshot;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::{future::Future, pin::Pin, sync::Mutex, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub trait AuthWindow: Send + Sync {
    /// Start the window immediately. Its closure must be observable by `closed`.
    fn open(&self, url: &str) -> Result<(), String>;
    /// Resolve when the user closes or cancels the authorization window.
    fn closed(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
    fn cancel(&self);
}

struct LoginGuard<'a, W: AuthWindow> {
    window: &'a W,
    active: &'a Mutex<Option<CancellationToken>>,
}
impl<W: AuthWindow> Drop for LoginGuard<'_, W> {
    fn drop(&mut self) {
        self.window.cancel();
        *self.active.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

pub fn callback_code(request: &str, state: &str) -> Option<String> {
    let parts: Vec<_> = request.split("\r\n").next()?.split_whitespace().collect();
    if parts.len() != 3 || parts[0] != "GET" || !parts[1].starts_with('/') {
        return None;
    }
    let url = Url::parse(&format!("http://127.0.0.1{}", parts[1])).ok()?;
    if url.path() != "/callback" {
        return None;
    }
    let params: Vec<_> = url.query_pairs().collect();
    let states: Vec<_> = params.iter().filter(|(k, _)| k == "state").collect();
    let codes: Vec<_> = params.iter().filter(|(k, _)| k == "code").collect();
    if states.len() != 1 || states[0].1 != state || codes.len() != 1 || codes[0].1.is_empty() {
        return None;
    }
    Some(codes[0].1.to_string())
}

pub struct WebLogin<W> {
    window: W,
    client: reqwest::Client,
    api_base: Url,
    cancellation: Mutex<Option<CancellationToken>>,
}
impl<W: AuthWindow> WebLogin<W> {
    pub fn new(window: W) -> Self {
        Self::with_api_base(
            window,
            Url::parse("https://api.pencil.dev/").expect("valid API URL"),
        )
    }
    /// Supply a local API server for offline integration tests.
    pub fn with_api_base(window: W, api_base: Url) -> Self {
        Self {
            window,
            api_base,
            client: reqwest::Client::new(),
            cancellation: Mutex::new(None),
        }
    }
    pub fn cancel(&self) {
        if let Some(token) = self
            .cancellation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            token.cancel();
        }
        self.window.cancel();
    }
    pub async fn sign_in(&self, device_id: &str, os_tag: &str) -> Result<TargetSnapshot, String> {
        let cancellation = CancellationToken::new();
        {
            let mut active = self.cancellation.lock().unwrap_or_else(|e| e.into_inner());
            if active.is_some() {
                return Err("正在添加账号".into());
            }
            *active = Some(cancellation.clone());
        }
        let _guard = LoginGuard {
            window: &self.window,
            active: &self.cancellation,
        };
        tokio::select! {
            result = self.sign_in_inner(device_id, os_tag) => result,
            _ = cancellation.cancelled() => Err("网页登录已取消或超时，原账号保持不变".into()),
        }
    }
    async fn post(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
        let response = self
            .client
            .post(
                self.api_base
                    .join(&format!("/auth/device-auth/{path}"))
                    .map_err(|e| e.to_string())?,
            )
            .timeout(Duration::from_secs(30))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err("网页登录接口请求失败，请稍后重试".into());
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "网页登录接口请求失败，请稍后重试")?;
        if !value.is_object() {
            return Err("网页登录接口请求失败，请稍后重试".into());
        }
        Ok(value)
    }
    async fn sign_in_inner(&self, device_id: &str, os_tag: &str) -> Result<TargetSnapshot, String> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|e| e.to_string())?;
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = Uuid::new_v4().to_string().to_uppercase();
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let start = self.post("start", serde_json::json!({"codeChallenge": challenge, "redirectUri": format!("http://127.0.0.1:{port}/callback"), "deviceId": device_id, "clientType": "desktop", "clientHost": os_tag})).await?;
        let base = Url::parse("https://pen.dev").map_err(|e| e.to_string())?;
        let mut url = base
            .join(
                start["loginUrl"]
                    .as_str()
                    .ok_or("登录接口返回了无效的网址")?,
            )
            .map_err(|_| "登录接口返回了无效的网址")?;
        if url.scheme() != "https" || !matches!(url.host_str(), Some("pen.dev" | "www.pen.dev")) {
            return Err("登录接口返回了无效的网址".into());
        }
        let query: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| k != "state")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        url.set_query(None);
        {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in query {
                pairs.append_pair(&k, &v);
            }
            pairs.append_pair("state", &state);
        }
        self.window.open(url.as_str())?;
        let callback = async {
            loop {
                let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
                let code = handle_callback(stream, Some(&state)).await;
                if let Some(code) = code {
                    break Ok::<String, String>(code);
                }
            }
        };
        let code = tokio::select! {
            biased;
            result = callback => result?,
            result = self.window.closed() => {
                result?;
                return Err("网页登录已取消或超时，原账号保持不变".into());
            }
        };
        let exchange = self.post("token", serde_json::json!({"code": code, "codeVerifier": verifier, "deviceId": device_id, "clientType": "desktop"}));
        // Keep rejecting callbacks while the accepted code is being exchanged.
        let reject_duplicates = async {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        handle_callback(stream, None).await;
                    }
                    Err(error) => break Err::<(), String>(error.to_string()),
                }
            }
        };
        let result = tokio::select! {
            result = exchange => result?,
            result = reject_duplicates => { result?; return Err("无法打开本机登录回调".into()); }
        };
        let email = result["email"]
            .as_str()
            .filter(|e| e.contains('@'))
            .ok_or("登录结果缺少账号信息")?;
        let token = result["sessionToken"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or("登录结果缺少账号信息")?;
        if result["needsCompletion"] == true {
            return Err("请先在 Pen 网页完成账号资料，再重新添加".into());
        }
        let mut session = serde_json::Map::new();
        session.insert("email".into(), email.into());
        session.insert("token".into(), token.into());
        if let Some(fields) = result.as_object() {
            for (key, value) in fields {
                if !["email", "sessionToken", "needsCompletion"].contains(&key.as_str())
                    && (value.is_string() || value.is_number() || value.is_boolean())
                {
                    session.insert(key.clone(), value.clone());
                }
            }
        }
        Ok(TargetSnapshot {
            session_file: serde_json::Value::Object(session).to_string(),
            email: email.into(),
            token: Some(token.into()),
        })
    }
}

async fn handle_callback(mut stream: TcpStream, state: Option<&str>) -> Option<String> {
    let read = async {
        let mut request = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let count = stream.read(&mut buffer).await.ok()?;
            if count == 0 {
                return None;
            }
            request.extend_from_slice(&buffer[..count]);
            if request.len() > 16384 {
                return None;
            }
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                return state
                    .and_then(|state| callback_code(&String::from_utf8_lossy(&request), state));
            }
        }
    };
    let code = tokio::time::timeout(Duration::from_secs(5), read)
        .await
        .ok()
        .flatten();
    let (status, body) = if code.is_some() {
        (
            "200 OK",
            "Login received. You can return to Pencil Account Switcher.",
        )
    } else {
        ("400 Bad Request", "Invalid or expired login callback.")
    };
    let reply = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    let _ = tokio::time::timeout(Duration::from_secs(2), stream.write_all(reply.as_bytes())).await;
    let _ = stream.shutdown().await;
    code
}
