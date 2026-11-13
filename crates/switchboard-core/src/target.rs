use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetSnapshot {
    pub session_file: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

pub type TargetFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub trait TargetAdapter: Send + Sync {
    fn live_identity(&self) -> Option<String>;
    fn capture_snapshot(&self) -> Result<TargetSnapshot, String>;
    fn apply_snapshot(&self, snapshot: &TargetSnapshot) -> Result<(), String>;
    fn is_running(&self) -> bool;
    fn terminate_and_wait(&self, timeout: Duration) -> TargetFuture<'_, bool>;
    fn can_launch(&self) -> bool;
    fn launch(&self) -> TargetFuture<'_, Result<(), String>>;
}

pub fn emails_match(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}
