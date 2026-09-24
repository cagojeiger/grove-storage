#![allow(clippy::unwrap_used)]
#[path = "storage_writes/authorization.rs"]
mod authorization;
#[path = "storage_writes/failures.rs"]
mod failures;
#[path = "storage_writes/lifecycle.rs"]
mod lifecycle;
mod support;

use filegate_db::{PgPool, management as db, registry, registry::StorageRow};
use grove_management_command::{Command, ErrorCode, Outcome, Output, input, model::StorageKind};
use grove_management_policy::{Role, Surface};
use grove_management_service::{Error, Proof, resources};
use support::*;

fn crypto() -> filegate_core::Crypto {
    filegate_core::Crypto::new("test", &"storage-test-root-at-least-32-bytes".into()).unwrap()
}
fn submission(id: &str, root: &str, capacity: i64) -> input::StorageInput {
    input::StorageInput {
        id: id.into(),
        spec: input::StorageSpec {
            kind: StorageKind::Fs,
            force_relay: false,
            root_path: Some(root.into()),
            endpoint: None,
            public_endpoint: None,
            region: None,
            bucket: None,
            force_path_style: false,
            access_key: None,
            secret_key: None,
            capacity_bytes: capacity,
        },
    }
}
// The service contract is tested independently of filesystem/network availability.
async fn verified(input: input::StorageInput) -> Result<StorageRow, Error> {
    Ok(StorageRow {
        id: input.id,
        kind: "fs".into(),
        force_relay: false,
        root_path: input.spec.root_path,
        endpoint: None,
        public_endpoint: None,
        region: None,
        bucket: None,
        force_path_style: false,
        access_key: None,
        secret_key_ciphertext: None,
        secret_key_nonce: None,
        enc_key_id: None,
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
        &verified(submission("local", "/fixture", 100))
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
