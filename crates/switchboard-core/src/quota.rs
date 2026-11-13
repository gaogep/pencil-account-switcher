use crate::target::TargetSnapshot;
use serde::Deserialize;
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, PartialEq)]
pub enum QuotaResult {
    Quota {
        used: i64,
        total: i64,
        plan: Option<String>,
    },
    Unsupported,
    Expired,
    Failure(String),
}

pub trait DeviceIdentity: Send + Sync {
    fn hostname(&self) -> String;
    fn os_tag(&self) -> &'static str;
    fn architecture(&self) -> &'static str;
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    is_metered: Option<bool>,
    used: Option<i64>,
    limit: Option<i64>,
}

pub fn parse_body(data: &[u8]) -> QuotaResult {
    let Ok(result) = serde_json::from_slice::<Response>(data) else {
        return QuotaResult::Failure("额度数据解析失败".into());
    };
    if result.is_metered == Some(false) {
        return QuotaResult::Unsupported;
    }
    match (result.used, result.limit) {
        (Some(used), Some(total)) => QuotaResult::Quota {
            used,
            total,
            plan: None,
        },
        _ => QuotaResult::Unsupported,
    }
}

pub fn status_result(status: u16) -> Option<QuotaResult> {
    match status {
        401 => Some(QuotaResult::Expired),
        403 => Some(QuotaResult::Failure(
            "没有权限读取额度，登录状态保持不变".into(),
        )),
        200..=299 => None,
        _ => Some(QuotaResult::Failure(format!("额度接口返回 {status}"))),
    }
}

pub struct PenQuotaProvider<P> {
    base_url: String,
    platform: P,
    device_id: OnceLock<String>,
    client: reqwest::Client,
}
impl<P: DeviceIdentity> PenQuotaProvider<P> {
    pub fn new(base_url: String, platform: P) -> Self {
        Self {
            base_url,
            platform,
            device_id: OnceLock::new(),
            client: reqwest::Client::new(),
        }
    }
    pub fn device_id(&self) -> &str {
        self.device_id.get_or_init(|| {
            format!(
                "{:x}",
                md5::compute(format!(
                    "{}{}{}",
                    self.platform.hostname(),
                    self.platform.os_tag(),
                    self.platform.architecture()
                ))
            )
        })
    }
    pub async fn fetch_quota(&self, snapshot: &TargetSnapshot) -> QuotaResult {
        let Some(token) = snapshot.token.as_ref() else {
            return QuotaResult::Expired;
        };
        let request = self.client.post(format!("{}/public/usage-status", self.base_url.trim_end_matches('/')))
            .timeout(Duration::from_secs(20)).bearer_auth(token).header("X-Device-Id", self.device_id())
            .json(&serde_json::json!({"featureKey": "mcp_usage_days", "timeZone": iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())}));
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => return QuotaResult::Failure(format!("网络错误：{e}")),
        };
        if let Some(result) = status_result(response.status().as_u16()) {
            return result;
        }
        match response.bytes().await {
            Ok(data) => parse_body(&data),
            Err(e) => QuotaResult::Failure(format!("网络错误：{e}")),
        }
    }
}
