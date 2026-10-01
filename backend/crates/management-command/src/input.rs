use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CommandError, ErrorCode, model::StorageKind};

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceInput {
    pub id: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataInput {
    pub id: String,
    #[schemars(with = "std::collections::BTreeMap<String, String>")]
    pub metadata: serde_json::Value,
}

impl Validate for MetadataInput {
    fn validate(&self) -> Result<(), CommandError> {
        require(resource_id(&self.id) && crate::metadata::valid_metadata(&self.metadata))
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorageInput {
    pub id: String,
    #[serde(deserialize_with = "storage_spec_object")]
    pub spec: StorageSpec,
}

// Serde structs also accept positional arrays; the wire contract uses objects.
fn storage_spec_object<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<StorageSpec, D::Error> {
    let fields = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
    serde_json::from_value(serde_json::Value::Object(fields)).map_err(serde::de::Error::custom)
}

/// Retains the existing storage submission shape. Provider requirements, URL
/// rules, access probes, and reference guards remain authoritative in service.
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorageSpec {
    #[serde(default, deserialize_with = "s3_kind")]
    #[schemars(extend("enum" = ["s3"]))]
    pub kind: StorageKind,
    #[serde(default)]
    pub force_relay: bool,
    pub root_path: Option<String>,
    pub endpoint: Option<String>,
    pub public_endpoint: Option<String>,
    pub region: Option<String>,
    pub bucket: Option<String>,
    #[serde(default)]
    pub force_path_style: bool,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub capacity_bytes: i64,
}

fn s3_kind<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<StorageKind, D::Error> {
    match StorageKind::deserialize(deserializer)? {
        StorageKind::S3 => Ok(StorageKind::S3),
        StorageKind::Fs => Err(serde::de::Error::custom(
            "only S3-compatible storage is supported",
        )),
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientCreateInput {
    pub id: String,
    pub storage_id: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientInput {
    pub client_id: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CredentialDeleteInput {
    pub client_id: String,
    pub access_key_id: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientKeyInput {
    pub client_id: String,
    pub key_hash: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryInput {
    #[serde(default = "default_history_days")]
    #[schemars(range(min = 1, max = 3650))]
    pub days: u16,
}

fn default_history_days() -> u16 {
    90
}

pub(crate) trait Validate {
    fn validate(&self) -> Result<(), CommandError>;
}

fn require(valid: bool) -> Result<(), CommandError> {
    if valid {
        Ok(())
    } else {
        Err(CommandError::rejected(ErrorCode::InvalidInput))
    }
}

// Lookup IDs retain the CLI's path-segment contract. Creation slug constraints
// and reserved Client names remain enforced by the registry service/database.
fn resource_id(value: &str) -> bool {
    !value.is_empty() && !matches!(value, "." | "..") && !value.chars().any(char::is_control)
}

fn key_hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

impl Validate for EmptyInput {
    fn validate(&self) -> Result<(), CommandError> {
        Ok(())
    }
}

impl Validate for ResourceInput {
    fn validate(&self) -> Result<(), CommandError> {
        require(resource_id(&self.id))
    }
}

impl Validate for StorageInput {
    fn validate(&self) -> Result<(), CommandError> {
        require(
            resource_id(&self.id)
                && self.spec.capacity_bytes >= 0
                && self.spec.kind == StorageKind::S3,
        )
    }
}

impl Validate for ClientCreateInput {
    fn validate(&self) -> Result<(), CommandError> {
        require(resource_id(&self.id) && resource_id(&self.storage_id))
    }
}

impl Validate for ClientInput {
    fn validate(&self) -> Result<(), CommandError> {
        require(resource_id(&self.client_id))
    }
}

impl Validate for CredentialDeleteInput {
    fn validate(&self) -> Result<(), CommandError> {
        require(
            resource_id(&self.client_id)
                && (8..=64).contains(&self.access_key_id.len())
                && self
                    .access_key_id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
        )
    }
}

impl Validate for ClientKeyInput {
    fn validate(&self) -> Result<(), CommandError> {
        require(resource_id(&self.client_id) && key_hash(&self.key_hash))
    }
}

impl Validate for HistoryInput {
    fn validate(&self) -> Result<(), CommandError> {
        require((1..=3650).contains(&self.days))
    }
}
