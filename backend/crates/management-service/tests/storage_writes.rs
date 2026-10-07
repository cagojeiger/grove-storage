#![allow(clippy::unwrap_used)]
#[path = "storage_writes/authorization.rs"]
mod authorization;
#[path = "storage_writes/checks.rs"]
mod checks;
#[path = "storage_writes/failures.rs"]
mod failures;
#[path = "storage_writes/lifecycle.rs"]
mod lifecycle;
mod support;

use grove_db::{PgPool, management as db, registry, registry::StorageRow};
use grove_management_command::{Command, ErrorCode, Outcome, Output, input, model::StorageKind};
use grove_management_policy::{Role, Surface};
use grove_management_service::{Error, Proof, resources};
use support::*;

fn crypto() -> grove_core::Crypto {
    grove_core::Crypto::new("test", &"storage-test-root-at-least-32-bytes".into()).unwrap()
}
fn submission(id: &str, root: &str, capacity: i64) -> input::StorageInput {
    input::StorageInput {
        id: id.into(),
        spec: input::StorageSpec {
            kind: StorageKind::S3,
            force_relay: false,
            root_path: None,
            endpoint: Some(format!("https://storage.test{root}")),
            public_endpoint: Some(format!("https://storage.test{root}")),
            region: Some("test".into()),
            bucket: Some("test".into()),
            force_path_style: false,
            access_key: None,
            secret_key: None,
            capacity_bytes: capacity,
        },
    }
}
// The service contract is tested independently of filesystem/network availability.
async fn verified(operation: resources::StorageOperation) -> Result<StorageRow, Error> {
    let resources::StorageOperation::Register(input) = operation else {
        return Err(Error::InvalidInput);
    };
    Ok(StorageRow {
        id: input.id,
        kind: "s3".into(),
        force_relay: false,
        endpoint: input.spec.endpoint,
        public_endpoint: input.spec.public_endpoint,
        region: Some("test".into()),
        bucket: Some("test".into()),
        force_path_style: false,
        access_key: Some("test".into()),
        secret_key_ciphertext: Some(vec![0; 32]),
        secret_key_nonce: Some(vec![0; 12]),
        enc_key_id: Some("test".into()),
        capacity_bytes: input.spec.capacity_bytes,
    })
}
async fn execute(pool: &PgPool, token: &str, command: Command) -> resources::Execution {
    resources::execute(
        pool,
        &crypto(),
        verified,
        Proof::Token(token),
        Surface::Cli,
        command,
    )
    .await
}
async fn seed(pool: &PgPool) {
    registry::insert_storage(
        pool,
        &verified(resources::StorageOperation::Register(submission(
            "local", "/fixture", 100,
        )))
        .await
        .unwrap(),
    )
    .await
    .unwrap();
}
fn mutations() -> Vec<Command> {
    vec![
        Command::StorageCreate(submission("new", "/new", 1)),
        Command::StorageReplace(submission("local", "/changed", 200)),
        Command::StorageDelete(input::ResourceInput { id: "local".into() }),
    ]
}
