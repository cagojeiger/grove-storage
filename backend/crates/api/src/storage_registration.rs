//! Shared storage registration, provider probes and credential encryption.

use axum::http::StatusCode;
use filegate_core::time::Clock;
use filegate_core::{Crypto, SecretString};
use filegate_db::PgPool;
use filegate_db::registry::{self, StorageRow};
use filegate_infra::{S3StorageSpec, s3_connect, s3_connect_with_clock};
use serde::Deserialize;
use std::sync::Arc;

use crate::error::{ApiError, bad_request};
use crate::storage_access::backend_from_row;
use filegate_infra::backend::StorageBackend;

/// Shared registration fields; legacy JSON defaults remain compatible.
#[derive(Deserialize)]
pub(crate) struct Submission {
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    force_relay: bool,
    root_path: Option<String>,
    endpoint: Option<String>,
    /// 서명 URL/전송 주체가 접근할 공개 주소. 생략하면 endpoint와 같다.
    public_endpoint: Option<String>,
    region: Option<String>,
    bucket: Option<String>,
    #[serde(default)]
    force_path_style: bool,
    access_key: Option<String>,
    secret_key: Option<SecretString>,
    capacity_bytes: i64,
}

fn default_kind() -> String {
    "s3".to_owned()
}

/// Reuse the shared field rules, backend probes and encryption without exposing
/// provider diagnostics through the common command contract.
pub(crate) async fn verify_command(
    crypto: &Crypto,
    clock: Arc<dyn Clock>,
    relay_base_ready: bool,
    operation: grove_management_service::resources::StorageOperation,
) -> Result<StorageRow, grove_management_service::Error> {
    use grove_management_service::{Error, resources::StorageOperation};
    let input = match operation {
        StorageOperation::Register(input) => input,
        StorageOperation::Test(row) => {
            if row.kind != "s3" {
                return Err(Error::InvalidInput);
            }
            let StorageBackend { spec, .. } =
                backend_from_row(crypto, &row).map_err(|_| Error::Unavailable)?;
            // Neither provider diagnostics nor credentials cross this boundary.
            s3_connect_with_clock(&spec, clock)
                .await
                .map_err(|_| Error::Unavailable)?;
            return Ok(row);
        }
    };
    let spec = input.spec;
    let body = Submission {
        kind: match spec.kind {
            grove_management_command::model::StorageKind::S3 => "s3",
            grove_management_command::model::StorageKind::Fs => "fs",
        }
        .into(),
        force_relay: spec.force_relay,
        root_path: spec.root_path,
        endpoint: spec.endpoint,
        public_endpoint: spec.public_endpoint,
        region: spec.region,
        bucket: spec.bucket,
        force_path_style: spec.force_path_style,
        access_key: spec.access_key,
        secret_key: spec.secret_key.map(SecretString::from),
        capacity_bytes: spec.capacity_bytes,
    };
    verified_row(crypto, clock, relay_base_ready, &input.id, body)
        .await
        .map_err(|error| match error {
            ApiError::Status(StatusCode::BAD_REQUEST, _) => {
                grove_management_service::Error::InvalidInput
            }
            _ => grove_management_service::Error::Unavailable,
        })
}

pub(crate) async fn verified_row(
    crypto: &Crypto,
    clock: Arc<dyn Clock>,
    relay_base_ready: bool,
    id: &str,
    body: Submission,
) -> Result<StorageRow, ApiError> {
    if body.capacity_bytes < 0 {
        return Err(bad_request("capacity_bytes must be >= 0"));
    }
    match body.kind.as_str() {
        "s3" => verified_s3_row(crypto, clock, relay_base_ready, id, body).await,
        _ => Err(bad_request("only S3-compatible storage is supported")),
    }
}

async fn verified_s3_row(
    crypto: &Crypto,
    clock: Arc<dyn Clock>,
    relay_base_ready: bool,
    id: &str,
    body: Submission,
) -> Result<StorageRow, ApiError> {
    let submission = validated_s3_submission(relay_base_ready, body)?;
    if let Err(error) = s3_connect_with_clock(&submission.spec, clock).await {
        // Provider errors may contain submitted addresses or credentials.
        tracing::error!(event = "storage.verify_failed", storage = %id, kind = "s3");
        return Err(bad_request(&format!(
            "storage verification failed: {error}"
        )));
    }
    encrypted_s3_row(crypto, id, submission)
}

/// 검증을 통과한 s3 제출물 — 아직 접근 확인 전.
struct S3Submission {
    spec: S3StorageSpec,
    force_relay: bool,
    capacity_bytes: i64,
}

fn validated_s3_submission(
    relay_base_ready: bool,
    body: Submission,
) -> Result<S3Submission, ApiError> {
    if body.root_path.as_deref().is_some_and(|v| !v.is_empty()) {
        return Err(bad_request("s3 storage does not take root_path"));
    }
    if body.force_relay && !relay_base_ready {
        return Err(bad_request(
            "relay storage requires FILEGATE_PUBLIC_URL to be configured",
        ));
    }
    let endpoint = require(body.endpoint, "endpoint")?;
    let public_endpoint = body
        .public_endpoint
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| endpoint.clone());
    require_http_url(&endpoint, "endpoint")?;
    require_http_url(&public_endpoint, "public_endpoint")?;
    Ok(S3Submission {
        spec: S3StorageSpec {
            endpoint,
            public_endpoint,
            region: require(body.region, "region")?,
            bucket: require(body.bucket, "bucket")?,
            force_path_style: body.force_path_style,
            access_key: require(body.access_key, "access_key")?,
            secret_key: body
                .secret_key
                .ok_or_else(|| bad_request("s3 storage requires secret_key"))?,
        },
        force_relay: body.force_relay,
        capacity_bytes: body.capacity_bytes,
    })
}

fn encrypted_s3_row(
    crypto: &Crypto,
    id: &str,
    submission: S3Submission,
) -> Result<StorageRow, ApiError> {
    let S3Submission {
        spec,
        force_relay,
        capacity_bytes,
    } = submission;
    let encrypted = crypto.encrypt(id, &spec.secret_key)?;
    Ok(StorageRow {
        id: id.to_owned(),
        kind: "s3".to_owned(),
        force_relay,
        endpoint: Some(spec.endpoint),
        public_endpoint: Some(spec.public_endpoint),
        region: Some(spec.region),
        bucket: Some(spec.bucket),
        force_path_style: spec.force_path_style,
        access_key: Some(spec.access_key),
        secret_key_ciphertext: Some(encrypted.ciphertext),
        secret_key_nonce: Some(encrypted.nonce),
        enc_key_id: Some(crypto.active_key_id().to_owned()),
        capacity_bytes,
    })
}

fn require(value: Option<String>, field: &str) -> Result<String, ApiError> {
    value
        .filter(|v| !v.is_empty())
        .ok_or_else(|| bad_request(&format!("s3 storage requires {field}")))
}

/// 한 storage의 접근 재검증 결과. status 서브커맨드가 storage별로 보고할 수
/// 있게, 첫 실패에 멈추지 않고 전부 모으는 check_registered가 돌려준다.
pub struct StorageCheck {
    pub id: String,
    pub kind: String,
    /// 접근 실패 사유. ok면 None.
    pub detail: Option<String>,
}

impl StorageCheck {
    pub fn ok(&self) -> bool {
        self.detail.is_none()
    }
}

/// 등록된 모든 storage의 접근을 재검증하되, 첫 실패에 멈추지 않고 결과를
/// 전부 모은다 (부팅용 strict 래퍼는 verify_registered).
pub async fn check_registered(pool: &PgPool, crypto: &Crypto) -> anyhow::Result<Vec<StorageCheck>> {
    let mut checks = Vec::new();
    for row in registry::list_storages(pool).await? {
        let detail = match backend_from_row(crypto, &row) {
            Err(error) => Some(error.to_string()),
            Ok(StorageBackend { spec, .. }) => s3_connect(&spec).await.err().map(|e| e.to_string()),
        };
        checks.push(StorageCheck {
            id: row.id,
            kind: row.kind,
            detail,
        });
    }
    Ok(checks)
}

/// 부팅 재검증 — 등록된 모든 storage의 접근을 확인한다 (ADR 001).
/// 실패하면 부팅 중단. 잘못된 마스터 키 설정도 여기서 잡힌다 (spec 01).
pub async fn verify_registered(pool: &PgPool, crypto: &Crypto) -> anyhow::Result<()> {
    for check in check_registered(pool, crypto).await? {
        match check.detail {
            Some(detail) => {
                anyhow::bail!("storage '{}' re-verification: {detail}", check.id)
            }
            None => {
                tracing::info!(event = "storage.connected", storage = %check.id, kind = %check.kind)
            }
        }
    }
    Ok(())
}

/// http(s) URL 형식 검사 — presign이 이 주소로 서명하므로 등록에서 거른다.
fn require_http_url(value: &str, field: &str) -> Result<(), ApiError> {
    let parsed: Result<axum::http::Uri, _> = value.parse();
    let valid = parsed
        .map(|uri| matches!(uri.scheme_str(), Some("http" | "https")) && uri.host().is_some())
        .unwrap_or(false);
    if valid {
        Ok(())
    } else {
        Err(bad_request(&format!("{field} must be an http(s) URL")))
    }
}

#[cfg(test)]
mod tests;
