//! Read-only registry-to-backend adapter shared by management and object paths.
//! Decrypts provider credentials without probing storage or changing metadata.

use filegate_core::{Crypto, EncryptedSecret};
use filegate_db::registry::StorageRow;
use filegate_infra::{S3StorageSpec, backend::StorageBackend};

pub fn backend_from_row(
    crypto: &Crypto,
    row: &StorageRow,
) -> filegate_core::Result<StorageBackend> {
    match row.kind.as_str() {
        "s3" => {
            let secret_key = crypto.decrypt(
                row.enc_key_id
                    .as_deref()
                    .ok_or_else(|| missing(row, "enc_key_id"))?,
                &row.id,
                &EncryptedSecret {
                    ciphertext: row
                        .secret_key_ciphertext
                        .clone()
                        .ok_or_else(|| missing(row, "secret_key_ciphertext"))?,
                    nonce: row
                        .secret_key_nonce
                        .clone()
                        .ok_or_else(|| missing(row, "secret_key_nonce"))?,
                },
            )?;
            Ok(StorageBackend::S3 {
                spec: S3StorageSpec {
                    endpoint: field(row, row.endpoint.clone(), "endpoint")?,
                    public_endpoint: field(row, row.public_endpoint.clone(), "public_endpoint")?,
                    region: field(row, row.region.clone(), "region")?,
                    bucket: field(row, row.bucket.clone(), "bucket")?,
                    force_path_style: row.force_path_style,
                    access_key: field(row, row.access_key.clone(), "access_key")?,
                    secret_key,
                },
                force_relay: row.force_relay,
            })
        }
        other => Err(filegate_core::Error::internal(format!(
            "storage '{}' has unknown kind '{other}'",
            row.id
        ))),
    }
}

fn field(row: &StorageRow, value: Option<String>, name: &str) -> filegate_core::Result<String> {
    value.ok_or_else(|| missing(row, name))
}

/// 종류별 필수는 DB CHECK가 집행하므로, 여기 도달하면 스키마 위반이다.
fn missing(row: &StorageRow, name: &str) -> filegate_core::Error {
    filegate_core::Error::internal(format!("storage '{}' is missing {name}", row.id))
}

#[cfg(test)]
mod tests;
