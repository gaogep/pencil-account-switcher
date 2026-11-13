use crate::{
    store::atomic_write,
    target::{emails_match, TargetAdapter, TargetFuture, TargetSnapshot},
};
use serde::Deserialize;
use std::{fs, path::PathBuf, time::Duration};

pub trait PenPlatform: Send + Sync {
    fn is_running(&self) -> bool;
    fn terminate_and_wait(&self, timeout: Duration) -> TargetFuture<'_, bool>;
    fn can_launch(&self) -> bool;
    fn launch(&self) -> TargetFuture<'_, Result<(), String>>;
}

#[derive(Deserialize)]
struct Session {
    email: Option<String>,
    token: Option<String>,
}
pub struct PenTargetAdapter<P> {
    session_path: PathBuf,
    platform: P,
}
impl<P> PenTargetAdapter<P> {
    pub fn new(home: PathBuf, platform: P) -> Self {
        Self {
            session_path: home.join(".pencil/session-desktop.json"),
            platform,
        }
    }
    fn read_session(&self) -> Result<(Vec<u8>, Session), String> {
        let data = fs::read(&self.session_path).map_err(|e| e.to_string())?;
        let session = serde_json::from_slice(&data).map_err(|e| format!("{e}"))?;
        Ok((data, session))
    }
}
impl<P: PenPlatform> TargetAdapter for PenTargetAdapter<P> {
    fn live_identity(&self) -> Option<String> {
        let (_, session) = self.read_session().ok()?;
        if session
            .token
            .as_ref()
            .is_some_and(|token| !token.is_empty())
        {
            session.email
        } else {
            None
        }
    }
    fn capture_snapshot(&self) -> Result<TargetSnapshot, String> {
        let (data, session) = self
            .read_session()
            .map_err(|_| "目标软件当前没有登录账号".to_string())?;
        let token = session
            .token
            .filter(|token| !token.is_empty())
            .ok_or("目标软件当前没有登录账号")?;
        let email = session.email.ok_or("目标软件当前没有登录账号")?;
        Ok(TargetSnapshot {
            session_file: String::from_utf8_lossy(&data).into_owned(),
            email,
            token: Some(token),
        })
    }
    fn apply_snapshot(&self, snapshot: &TargetSnapshot) -> Result<(), String> {
        let session: Session =
            serde_json::from_str(&snapshot.session_file).map_err(|e| e.to_string())?;
        let valid = session
            .email
            .as_ref()
            .is_some_and(|email| emails_match(email, &snapshot.email))
            && session
                .token
                .as_ref()
                .is_some_and(|token| !token.is_empty());
        if !valid {
            return Err(format!(
                "切换后身份校验失败（期望 {}，实际 {}）",
                snapshot.email,
                session.email.as_deref().unwrap_or("无")
            ));
        }
        atomic_write(&self.session_path, snapshot.session_file.as_bytes(), true)
            .map_err(|e| e.to_string())?;
        let actual = self.live_identity();
        if !actual
            .as_ref()
            .is_some_and(|email| emails_match(email, &snapshot.email))
        {
            return Err(format!(
                "切换后身份校验失败（期望 {}，实际 {}）",
                snapshot.email,
                actual.as_deref().unwrap_or("无")
            ));
        }
        Ok(())
    }
    fn is_running(&self) -> bool {
        self.platform.is_running()
    }
    fn terminate_and_wait(&self, timeout: Duration) -> TargetFuture<'_, bool> {
        self.platform.terminate_and_wait(timeout)
    }
    fn can_launch(&self) -> bool {
        self.platform.can_launch()
    }
    fn launch(&self) -> TargetFuture<'_, Result<(), String>> {
        self.platform.launch()
    }
}
