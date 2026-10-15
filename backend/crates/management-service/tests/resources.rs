#![allow(clippy::unwrap_used)]
#[path = "resources/authorization.rs"]
mod authorization;
#[path = "resources/failures.rs"]
mod failures;
mod support;

use grove_db::{PgPool, management as db};
use grove_management_command::{Command, ErrorCode, Output, input};
use grove_management_policy::{Role, Surface};
use grove_management_service::{Proof, resources};
use support::*;

fn clients() -> Command {
    Command::ClientList(input::EmptyInput {})
}
fn crypto() -> grove_core::Crypto {
    grove_core::Crypto::new("test", &"resource-test-root-at-least-32-bytes".into()).unwrap()
}
fn keys() -> Command {
    Command::ClientKeyList(input::ClientInput {
        client_id: "app".into(),
    })
}
async fn seed(pool: &PgPool) {
    sqlx::raw_sql("INSERT INTO storages(id,kind,endpoint,public_endpoint,region,bucket,access_key,secret_key_ciphertext,secret_key_nonce,enc_key_id,capacity_bytes) VALUES('local','s3','https://storage.example','https://storage.example','us-east-1','objects','key',decode('01','hex'),decode(repeat('00',12),'hex'),'test',100);
        INSERT INTO clients(id,storage_id) VALUES('app','local');
        INSERT INTO client_native_keys(client_id,key_hash) VALUES('app','sha256:'||repeat('a',64));")
        .execute(pool).await.unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn resources_read_status_is_server_observation_without_history_growth(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = audit_count(&pool).await;
    let execution = resources::execute(
        &pool,
        &crypto(),
        unexpected_storage_probe,
        Proof::Token(&admin.token),
        Surface::Cli,
        Command::Status(input::EmptyInput {}),
    )
    .await;
    assert!(matches!(
        execution.result.unwrap(), Output::Status(status)
        if status.registry.storage_count == Some(1)
            && status.registry.client_count == Some(1)
            && matches!(status.storage_access, grove_management_command::model::StorageAccess::NotChecked)
    ));
    assert_eq!(audit_count(&pool).await, before);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(execution.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_resource_reads_keep_safe_targets_not_arbitrary_lookup_input(pool: PgPool) {
    let admin = owner(&pool).await;
    for id in ["missing", "private/lookup?token=sentinel"] {
        let execution = resources::execute(
            &pool,
            &crypto(),
            unexpected_storage_probe,
            Proof::Token(&admin.token),
            Surface::Cli,
            Command::StorageShow(input::ResourceInput { id: id.into() }),
        )
        .await;
        assert_eq!(execution.result.unwrap_err().code, ErrorCode::NotFound);
        let row: (Option<String>, Option<String>, String) = sqlx::query_as("SELECT target_type,target_id,stage FROM management.command_invocations WHERE request_id=$1")
            .bind(execution.request_id).fetch_one(&pool).await.unwrap();
        if id == "missing" {
            assert_eq!(
                row,
                (
                    Some("storage".into()),
                    Some("missing".into()),
                    "execution".into()
                )
            );
        } else {
            assert_eq!(row, (None, None, "execution".into()));
        }
    }
}
