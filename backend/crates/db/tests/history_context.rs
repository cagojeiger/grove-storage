#![allow(clippy::unwrap_used)]

use grove_db::{PgPool, management::telemetry::Target};

#[test]
fn resource_targets_accept_registered_slugs_not_lookup_payloads() {
    for id in ["s3", "backup-s3", &"a".repeat(64)] {
        assert!(Target::resource("storage", id).is_some());
        assert!(Target::resource("client", id).is_some());
    }
    for id in [
        "",
        "-app",
        "app-",
        "Uppercase",
        "../private",
        "https://private.example",
        "sha256:private",
        "private\nvalue",
        &"a".repeat(65),
    ] {
        assert!(Target::resource("storage", id).is_none());
    }
    assert!(Target::resource("credential", "private-key-value").is_none());
}

#[sqlx::test(migrations = false)]
async fn history_upgrade_preserves_rows_without_inventing_context(pool: PgPool) {
    sqlx::raw_sql(include_str!("../migrations/0001_time.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0004_management.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO management.command_invocations(actor_kind,request_id,surface,operation,outcome,duration_ms) VALUES('system',$1,'cli','storage.list','succeeded',0)")
        .bind(id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO management.security_events(actor_kind,request_id,surface,event_type,reason_code) VALUES('anonymous',$1,'cli','authentication_failed','unauthenticated')")
        .bind(id).execute(&pool).await.unwrap();
    let before: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT created_at FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/0009_management_history_context.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let after: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT created_at FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    let fields: (Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as("SELECT target_type,target_id,stage,auth_method FROM management.command_invocations WHERE request_id=$1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(fields, (None, None, None, None));
    let fields: (Option<String>, Option<String>, Option<String>, Option<bool>) = sqlx::query_as("SELECT operation,stage,auth_method,is_sampled FROM management.security_events WHERE request_id=$1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(fields, (None, None, None, None));
    let security_before: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM management.security_events e WHERE request_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0010_security_session_end.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let security_after: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM management.security_events e WHERE request_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(security_before, security_after);
    sqlx::query("INSERT INTO management.security_events(actor_kind,request_id,surface,event_type,reason_code) VALUES('system',$1,'console','session_ended','user_requested')")
        .bind(id).execute(&pool).await.unwrap();
    assert!(sqlx::query("INSERT INTO management.security_events(actor_kind,request_id,surface,event_type,reason_code) VALUES('system',$1,'console','unknown','user_requested')")
        .bind(id).execute(&pool).await.is_err());
    for (kind, target) in [
        (Some("storage"), None),
        (None, Some("private")),
        (Some("unknown"), Some("private")),
    ] {
        assert!(sqlx::query("UPDATE management.command_invocations SET target_type=$2,target_id=$3 WHERE request_id=$1")
            .bind(id).bind(kind).bind(target).execute(&pool).await.is_err());
    }
}
