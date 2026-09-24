#![allow(clippy::unwrap_used)]
#[path = "resource_writes/authorization.rs"]
mod authorization;
#[path = "resource_writes/failures.rs"]
mod failures;
#[path = "resource_writes/lifecycle.rs"]
mod lifecycle;
mod support;

use filegate_db::{PgPool, management as db};
use grove_management_command::{Command, ErrorCode, Outcome, Output, input};
use grove_management_policy::{Role, Surface};
use grove_management_service::{Proof, resources};
use support::*;

fn crypto() -> filegate_core::Crypto {
    filegate_core::Crypto::new("test", &"resource-test-root-at-least-32-bytes".into()).unwrap()
}
fn native_hash() -> String {
    format!("sha256:{}", "a".repeat(64))
}
async fn seed(pool: &PgPool) {
    sqlx::raw_sql("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('local','fs','/fixture',100);
        INSERT INTO clients(id,storage_id) VALUES('app','local');
        INSERT INTO client_keys(client_id,key_hash) VALUES('app','sha256:'||repeat('a',64));
        INSERT INTO s3_credentials(access_key_id,client_id,secret_key_ciphertext,secret_key_nonce,enc_key_id)
        VALUES('existingkey','app','ciphertext',decode(repeat('ab',12),'hex'),'test');")
        .execute(pool).await.unwrap();
}
fn mutations() -> Vec<Command> {
    vec![
        Command::ClientCreate(input::ClientCreateInput {
            id: "new".into(),
            storage_id: "local".into(),
        }),
        Command::ClientDelete(input::ResourceInput { id: "app".into() }),
        Command::ClientKeyRegister(input::ClientKeyInput {
            client_id: "app".into(),
            key_hash: format!("sha256:{}", "b".repeat(64)),
        }),
        Command::ClientKeyDelete(input::ClientKeyInput {
            client_id: "app".into(),
            key_hash: native_hash(),
        }),
        Command::CredentialCreate(input::ClientInput {
            client_id: "app".into(),
        }),
        Command::CredentialDelete(input::CredentialDeleteInput {
            client_id: "app".into(),
            access_key_id: "existingkey".into(),
        }),
    ]
}
async fn execute(pool: &PgPool, token: &str, command: Command) -> resources::Execution {
    resources::execute(pool, &crypto(), Proof::Token(token), Surface::Cli, command).await
}
async fn resource_counts(pool: &PgPool) -> (i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM clients),(SELECT count(*) FROM client_keys),(SELECT count(*) FROM s3_credentials)")
        .fetch_one(pool).await.unwrap()
}
