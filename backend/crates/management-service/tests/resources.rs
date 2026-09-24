#![allow(clippy::unwrap_used)]
#[path = "resources/authorization.rs"]
mod authorization;
#[path = "resources/failures.rs"]
mod failures;
mod support;

use filegate_db::{PgPool, management as db};
use grove_management_command::{Command, ErrorCode, Output, input};
use grove_management_policy::{Role, Surface};
use grove_management_service::{Proof, resources};
use support::*;

fn clients() -> Command {
    Command::ClientList(input::EmptyInput {})
}
fn crypto() -> filegate_core::Crypto {
    filegate_core::Crypto::new("test", &"resource-test-root-at-least-32-bytes".into()).unwrap()
}
fn keys() -> Command {
    Command::ClientKeyList(input::ClientInput {
        client_id: "app".into(),
    })
}
async fn seed(pool: &PgPool) {
    sqlx::raw_sql("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('local','fs','/fixture',100);
        INSERT INTO clients(id,storage_id) VALUES('app','local');
        INSERT INTO client_keys(client_id,key_hash) VALUES('app','sha256:'||repeat('a',64));")
        .execute(pool).await.unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn resources_read_status_is_server_observation_and_logs_once(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = audit_count(&pool).await;
    let execution = resources::execute(
        &pool,
        &crypto(),
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
    let (operation, surface, outcome): (String, String, String) = sqlx::query_as(
        "SELECT operation,surface,outcome FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(execution.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (operation.as_str(), surface.as_str(), outcome.as_str()),
        ("status", "cli", "succeeded")
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(execution.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}
