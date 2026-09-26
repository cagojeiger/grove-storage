#![allow(clippy::unwrap_used)]
mod support;
use filegate_db::PgPool;
use grove_management_command::{Command, ErrorCode, Outcome, decode};
use grove_management_policy::{Role, Surface};
use grove_management_service::{Proof, resources};
use serde_json::{Value, json};
use support::*;

async fn seed(pool: &PgPool) {
    sqlx::raw_sql("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('local','fs','/unchanged',100);
        INSERT INTO clients(id,storage_id) VALUES('app','local');")
        .execute(pool).await.unwrap();
}

async fn execute(
    pool: &PgPool,
    token: &str,
    surface: Surface,
    command: Command,
) -> resources::Execution {
    let crypto =
        filegate_core::Crypto::new("test", &"metadata-test-root-at-least-32-bytes".into()).unwrap();
    resources::execute(
        pool,
        &crypto,
        unexpected_storage_probe,
        Proof::Token(token),
        surface,
        command,
    )
    .await
}

fn read(resource: &str, id: &str) -> Command {
    decode(1, &format!("{resource}.metadata.show"), json!({"id":id})).unwrap()
}
fn replace(resource: &str, id: &str, metadata: Value) -> Command {
    decode(
        1,
        &format!("{resource}.metadata.replace"),
        json!({"id":id,"metadata":metadata}),
    )
    .unwrap()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn metadata_lifecycle_is_independent_of_settings_and_audited_without_payload(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    for (resource, id) in [("storage", "local"), ("client", "app")] {
        let result = execute(&pool, &admin.token, Surface::Cli, read(resource, id))
            .await
            .result
            .unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            json!({"id":id,"metadata":{}})
        );
        let labels = json!({"description":"private-text-not-for-audit", "environment":"home", "한글":"설명"});
        let execution = execute(
            &pool,
            &admin.token,
            Surface::Mcp,
            replace(resource, id, labels.clone()),
        )
        .await;
        assert_eq!(
            serde_json::to_value(execution.result.unwrap()).unwrap(),
            json!({"id":id,"metadata":labels})
        );
        let audit: (String, String, String, Value) = sqlx::query_as("SELECT action,resource_type,resource_id,metadata FROM management.audit_events WHERE request_id=$1")
            .bind(execution.request_id).fetch_one(&pool).await.unwrap();
        assert_eq!(
            audit,
            (
                format!("{resource}.metadata.replace"),
                resource.into(),
                id.into(),
                json!({})
            )
        );
        let output = execute(&pool, &admin.token, Surface::Cli, read(resource, id))
            .await
            .result
            .unwrap();
        assert_eq!(
            serde_json::to_value(output).unwrap().get("metadata"),
            Some(&labels)
        );
        let output = execute(
            &pool,
            &admin.token,
            Surface::Cli,
            replace(resource, id, json!({})),
        )
        .await
        .result
        .unwrap();
        assert_eq!(
            serde_json::to_value(output).unwrap().get("metadata"),
            Some(&json!({}))
        );
    }
    let row: (String, i64, String) = sqlx::query_as("SELECT s.root_path,s.capacity_bytes,c.storage_id FROM storages s JOIN clients c ON c.storage_id=s.id")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(row, ("/unchanged".into(), 100, "local".into()));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn readers_can_read_but_cannot_replace_and_missing_resources_return_not_found(pool: PgPool) {
    let _admin = owner(&pool).await;
    let reader = user_login(&pool, Role::Reader, 2).await;
    let writer = user_login(&pool, Role::Writer, 3).await;
    seed(&pool).await;
    for (resource, id) in [("storage", "local"), ("client", "app")] {
        for surface in [Surface::Cli, Surface::Mcp] {
            assert!(
                execute(&pool, &reader.token, surface, read(resource, id))
                    .await
                    .result
                    .is_ok()
            );
            let error = execute(
                &pool,
                &reader.token,
                surface,
                replace(resource, id, json!({"x":"y"})),
            )
            .await
            .result
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::Forbidden);
            assert_eq!(error.outcome, Outcome::NotApplied);
            assert!(
                execute(
                    &pool,
                    &writer.token,
                    surface,
                    replace(resource, id, json!({"x":"y"}))
                )
                .await
                .result
                .is_ok()
            );
            for command in [
                read(resource, "missing"),
                replace(resource, "missing", json!({})),
            ] {
                assert_eq!(
                    execute(&pool, &writer.token, surface, command)
                        .await
                        .result
                        .unwrap_err()
                        .code,
                    ErrorCode::NotFound
                );
            }
        }
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn audit_failure_rolls_back_labels(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    reject_audit(&pool).await;
    for (resource, id) in [("storage", "local"), ("client", "app")] {
        let error = execute(
            &pool,
            &admin.token,
            Surface::Cli,
            replace(resource, id, json!({"x":"y"})),
        )
        .await
        .result
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Unavailable);
        assert_eq!(error.outcome, Outcome::NotApplied);
        let output = execute(&pool, &admin.token, Surface::Cli, read(resource, id))
            .await
            .result
            .unwrap();
        assert_eq!(
            serde_json::to_value(output).unwrap().get("metadata"),
            Some(&json!({}))
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn database_enforces_shape_and_exact_normalized_byte_limit(pool: PgPool) {
    seed(&pool).await;
    for sql in [
        "UPDATE storages SET metadata=$1",
        "UPDATE clients SET metadata=$1",
    ] {
        for value in [
            json!(null),
            json!([]),
            json!({"x":1}),
            json!({"x":{}}),
            json!({"x":[]}),
            json!({"x":false}),
            json!({"x":"a".repeat(8184)}),
        ] {
            assert!(sqlx::query(sql).bind(value).execute(&pool).await.is_err());
        }
        for value in [
            json!({}),
            json!({"x":"a".repeat(8183)}),
            json!({"x":"한".repeat(2727)}),
        ] {
            sqlx::query(sql).bind(value).execute(&pool).await.unwrap();
        }
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn existing_rows_upgrade_to_empty_metadata_and_legacy_updates_preserve_it(pool: PgPool) {
    sqlx::raw_sql(
        "ALTER TABLE storages DROP COLUMN metadata; ALTER TABLE clients DROP COLUMN metadata;",
    )
    .execute(&pool)
    .await
    .unwrap();
    seed(&pool).await;
    sqlx::raw_sql(include_str!(
        "../../db/migrations/0015_resource_metadata.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let initial: (Value, Value) = sqlx::query_as(
        "SELECT s.metadata,c.metadata FROM storages s JOIN clients c ON c.storage_id=s.id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(initial, (json!({}), json!({})));
    sqlx::query("UPDATE storages SET metadata=$1")
        .bind(json!({"x":"y"}))
        .execute(&pool)
        .await
        .unwrap();
    let mut row = filegate_db::registry::get_storage(&pool, "local")
        .await
        .unwrap()
        .unwrap();
    row.capacity_bytes = 200;
    filegate_db::registry::update_storage(&pool, &row)
        .await
        .unwrap();
    let metadata: Value = sqlx::query_scalar("SELECT metadata FROM storages WHERE id='local'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(metadata, json!({"x":"y"}));
}
