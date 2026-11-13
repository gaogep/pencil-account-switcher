use crate::{
    store::{atomic_write, DataDirectory},
    target::TargetFuture,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

const PUBLIC_KEY: &str = "i4OQxXFQgGlNb+T2kwMmSawM6Ps+2YsU1uvtLsPGvpc=";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppRelease {
    pub version: String,
    pub asset_url: String,
    pub signature_url: String,
    pub page_url: String,
}
#[derive(Clone, Copy)]
pub enum PlatformAsset {
    Mac,
    Windows,
}
#[derive(Deserialize)]
struct Payload {
    tag_name: String,
    html_url: String,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

pub fn normalize(version: &str) -> &str {
    version.trim().trim_start_matches(['v', 'V'])
}
pub fn compare(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| -> Vec<i64> {
        normalize(v)
            .split('.')
            .filter(|x| !x.is_empty())
            .map(|x| x.parse().unwrap_or(0))
            .collect()
    };
    let a = parse(a);
    let b = parse(b);
    for i in 0..a.len().max(b.len()) {
        match a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)) {
            Ordering::Equal => (),
            other => return other,
        }
    }
    Ordering::Equal
}

pub async fn fetch_latest(
    client: &reqwest::Client,
    repo: &str,
    platform: PlatformAsset,
) -> Result<AppRelease, String> {
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Pencil-Account-Switcher")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let payload: Payload = response.json().await.map_err(|e| e.to_string())?;
    if payload.prerelease {
        return Err("发布版本无效".into());
    }
    let asset = payload
        .assets
        .iter()
        .find(|asset| {
            let name = asset.name.to_lowercase();
            match platform {
                PlatformAsset::Mac => name.contains("arm64") && name.ends_with(".zip"),
                PlatformAsset::Windows => name.ends_with("-windows-x64-setup.exe"),
            }
        })
        .ok_or("发布缺少更新包")?;
    let signature = payload
        .assets
        .iter()
        .find(|x| x.name == format!("{}.sig", asset.name))
        .ok_or("发布缺少签名文件")?;
    Ok(AppRelease {
        version: normalize(&payload.tag_name).into(),
        asset_url: asset.browser_download_url.clone(),
        signature_url: signature.browser_download_url.clone(),
        page_url: payload.html_url,
    })
}

pub fn verify_signature(data: &[u8], signature: &str) -> Result<(), String> {
    let fail = || "更新包签名校验失败，已中止安装".to_string();
    let key = STANDARD
        .decode(PUBLIC_KEY)
        .map_err(|_| "缺少更新公钥，无法校验更新包")?;
    let key: [u8; 32] = key.try_into().map_err(|_| "缺少更新公钥，无法校验更新包")?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| "缺少更新公钥，无法校验更新包")?;
    let bytes = STANDARD.decode(signature.trim()).map_err(|_| fail())?;
    let signature = Signature::from_slice(&bytes).map_err(|_| fail())?;
    key.verify(data, &signature).map_err(|_| fail())
}

pub async fn download(
    client: &reqwest::Client,
    url: &str,
    cancel: &CancellationToken,
    progress: impl Fn(u64, Option<u64>),
) -> Result<Vec<u8>, String> {
    let response = tokio::select! { r = client.get(url).send() => r.map_err(|e| e.to_string())?, _ = cancel.cancelled() => return Err("已取消".into()) };
    if !response.status().is_success() {
        return Err(format!(
            "下载更新失败（HTTP {}）",
            response.status().as_u16()
        ));
    }
    let total = response.content_length();
    let mut stream = response.bytes_stream();
    let mut data = Vec::new();
    loop {
        let chunk = tokio::select! { chunk = stream.next() => chunk, _ = cancel.cancelled() => return Err("已取消".into()) };
        match chunk {
            Some(Ok(chunk)) => {
                data.extend_from_slice(&chunk);
                progress(data.len() as u64, total);
            }
            Some(Err(e)) => return Err(e.to_string()),
            None => return Ok(data),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTimes {
    last_check: Option<u64>,
    last_attempt: Option<u64>,
}
pub struct UpdateSchedule {
    path: PathBuf,
    times: UpdateTimes,
}
impl UpdateSchedule {
    pub fn new(platform: &impl DataDirectory) -> Self {
        let path = platform.data_dir().join("state.json");
        let times = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, times }
    }
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    pub fn should_check(&self, force: bool) -> bool {
        let now = Self::now();
        force
            || (self
                .times
                .last_check
                .map_or(true, |t| now.saturating_sub(t) >= 86400)
                && self
                    .times
                    .last_attempt
                    .map_or(true, |t| now.saturating_sub(t) >= 3600))
    }
    pub fn attempt(&mut self) -> std::io::Result<()> {
        self.times.last_attempt = Some(Self::now());
        self.save()
    }
    pub fn success(&mut self) -> std::io::Result<()> {
        self.times.last_check = Some(Self::now());
        self.save()
    }
    fn save(&self) -> std::io::Result<()> {
        atomic_write(
            &self.path,
            &serde_json::to_vec(&self.times).map_err(std::io::Error::other)?,
            false,
        )
    }
}

pub trait UpdateInstaller: Send + Sync {
    fn prepare<'a>(
        &'a self,
        package: &'a Path,
        version: &'a str,
    ) -> TargetFuture<'a, Result<PathBuf, String>>;
    fn install<'a>(&'a self, prepared: &'a Path) -> TargetFuture<'a, Result<(), String>>;
}
