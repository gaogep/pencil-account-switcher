use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthState {
    Ok,
    Expired,
    #[default]
    #[serde(other)]
    Unknown,
}

fn serialize_uuid<S: Serializer>(id: &Uuid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&id.hyphenated().to_string().to_uppercase())
}
fn deserialize_uuid<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Uuid, D::Error> {
    let value = String::deserialize(deserializer)?;
    Uuid::parse_str(&value).map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRecord {
    #[serde(
        serialize_with = "serialize_uuid",
        deserialize_with = "deserialize_uuid"
    )]
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub is_current: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_days: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_days: Option<i64>,
    /// Seconds since 2001-01-01, matching Foundation's default Date encoding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_quota_update: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub auth_state: AuthState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreFile {
    pub store_version: i64,
    pub accounts: Vec<AccountRecord>,
}
impl Default for StoreFile {
    fn default() -> Self {
        Self {
            store_version: 1,
            accounts: Vec::new(),
        }
    }
}
