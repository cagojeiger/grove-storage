use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StorageKind {
    #[default]
    S3,
    Fs,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Storage {
    pub id: String,
    pub kind: StorageKind,
    pub force_relay: bool,
    pub root_path: Option<String>,
    pub endpoint: Option<String>,
    pub public_endpoint: Option<String>,
    pub region: Option<String>,
    pub bucket: Option<String>,
    pub force_path_style: bool,
    pub access_key: Option<String>,
    pub capacity_bytes: i64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Client {
    pub id: String,
    pub storage_id: String,
}

/// Point-in-time internal bucket access, not object transfer or public URL health.
#[derive(Deserialize, Serialize, JsonSchema)]
pub struct StorageConnection {
    pub id: String,
    pub state: State,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ClientKey {
    pub client_id: String,
    pub key_hash: String,
}

/// One-time wire response. Do not serialize into history or diagnostic logs.
#[derive(Deserialize, Serialize, JsonSchema)]
pub struct IssuedCredential {
    pub access_key_id: String,
    pub secret_key: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Deleted {
    pub resource: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct StorageUsage {
    pub storage_id: String,
    pub kind: StorageKind,
    pub capacity_bytes: i64,
    pub reserved_bytes: i64,
    pub active_bytes: i64,
    pub purge_pending_bytes: i64,
    pub remaining_bytes: i64,
    pub reserved_files: i64,
    pub active_files: i64,
    pub purge_pending_files: i64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct ClientUsage {
    pub client_id: String,
    pub storage_id: String,
    pub active_files: i64,
    pub active_bytes: i64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Snapshot {
    pub day: String,
    pub storage_id: String,
    pub client_id: String,
    pub active_bytes: i64,
    pub active_files: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Ok,
    Failed,
    Unknown,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Registry {
    pub state: State,
    pub usage: State,
    pub clients: State,
    pub storage_count: Option<usize>,
    pub client_count: Option<usize>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StorageAccess {
    NotChecked,
}

/// Registry readiness, not a physical-storage or end-to-end availability proof.
#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Status {
    pub server_version: Option<String>,
    pub identity: State,
    pub health: State,
    pub readiness: State,
    pub registry: Registry,
    pub storage_access: StorageAccess,
}
