use crate::target::TargetSnapshot;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::{future::Future, pin::Pin, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub trait AuthWindow: Send + Sync {
    fn open<'a>(
        &'a self,
        url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
    fn cancel(&self);
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
    cancellation: CancellationToken,
}
impl<W: AuthWindow> WebLogin<W> {
    pub fn new(window: W) -> Self {
        Self {
            window,
            client: reqwest::Client::new(),
            cancellation: CancellationToken::new(),
        }
    }
    pub fn cancel(&self) {
        self.cancellation.cancel();
        self.window.cancel();
    }
    pub async fn sign_in(&self, device_id: &str, os_tag: &str) -> Result<TargetSnapshot, String> {
        let result = tokio::select! {
            result = self.sign_in_inner(device_id, os_tag) => result,
            _ = self.cancellation.cancelled() => Err("登录已取消".into()),
        };
        self.window.cancel();
        result
    }
    async fn post(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
        let response = self
            .client
            .post(format!("https://api.pencil.dev/auth/device-auth/{path}"))
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
        self.window.open(url.as_str()).await?;
        let code = loop {
            let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut request = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let count = stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if request.len() > 16384 {
                    break;
                }
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            if request.len() > 16384 || !request.windows(4).any(|w| w == b"\r\n\r\n") {
                continue;
            }
            let code = callback_code(&String::from_utf8_lossy(&request), &state);
            let body = if code.is_some() {
                "Login received. You can return to Pencil Account Switcher."
            } else {
                "Invalid or expired login callback."
            };
            let status = if code.is_some() {
                "200 OK"
            } else {
                "400 Bad Request"
            };
            let reply = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            let _ = stream.write_all(reply.as_bytes()).await;
            if let Some(code) = code {
                break code;
            }
        };
        let result = self.post("token", serde_json::json!({"code": code, "codeVerifier": verifier, "deviceId": device_id, "clientType": "desktop"})).await?;
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
