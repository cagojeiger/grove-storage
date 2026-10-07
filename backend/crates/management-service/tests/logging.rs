#![allow(clippy::unwrap_used)]
mod support;
use grove_db::{
    PgPool,
    management::{self as db, NewAccount, admission},
};
use grove_management_policy::{Role, Surface};
use grove_management_service::{self as service, Command, Error, Output, Page, Proof};
use support::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn mutations_audit_once_and_reads_only_record_invocations(pool: PgPool) {
    let admin = owner(&pool).await;
    let before = audit_count(&pool).await;
    let created = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::CreateAccount(NewAccount {
            display_name: "new-user",
            role: Role::Reader,
        }),
    )
    .await;
    assert!(created.result.is_ok());
    assert_eq!(audit_count(&pool).await, before + 1);
    let actors: (uuid::Uuid, uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT actor_id,credential_id,session_id FROM management.audit_events WHERE request_id=$1",
    )
    .bind(created.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actors, (admin.account, admin.credential, admin.session_id));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations WHERE request_id=$1 AND outcome='succeeded'").bind(created.request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert!(
        service::execute(
            &pool,
            Proof::Session(&admin.session),
            Surface::Console,
            Command::Accounts(Page::default().into())
        )
        .await
        .result
        .is_ok()
    );
    assert_eq!(audit_count(&pool).await, before + 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn invalid_proof_logs_anonymous_security_without_payload(pool: PgPool) {
    let secret = "sentinel-raw-secret-must-not-be-logged";
    let result = service::execute(
        &pool,
        Proof::Token(secret),
        Surface::Mcp,
        Command::Accounts(Page::default().into()),
    )
    .await;
    assert!(matches!(result.result, Err(Error::Unauthenticated)));
    let event: String = sqlx::query_scalar(
        "SELECT row_to_json(e)::text FROM management.security_events e WHERE request_id=$1",
    )
    .bind(result.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(event.contains("anonymous"));
    assert!(!event.contains(secret));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn token_denial_retains_user_and_credential_without_creating_audit(pool: PgPool) {
    let admin = owner(&pool).await;
    let automation_user = admin.account;
    db::issue_credential(&pool, &context(), automation_user, &key(&hash(2)))
        .await
        .unwrap();
    let before = audit_count(&pool).await;
    let denied = service::execute(
        &pool,
        Proof::Token(&hash(2)),
        Surface::Mcp,
        Command::Audit(Page::default().into()),
    )
    .await;
    assert!(matches!(denied.result, Err(Error::Forbidden)));
    let row: (String, uuid::Uuid, uuid::Uuid) = sqlx::query_as("SELECT actor_kind,actor_id,credential_id FROM management.command_invocations WHERE request_id=$1").bind(denied.request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, "account");
    assert_eq!(row.1, admin.account);
    assert_ne!(row.2, admin.credential);
    assert_eq!(audit_count(&pool).await, before);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn audit_failure_rejects_change_but_invocation_failure_does_not(pool: PgPool) {
    let admin = owner(&pool).await;
    reject_audit(&pool).await;
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::CreateAccount(NewAccount {
            display_name: "rollback",
            role: Role::Reader,
        }),
    )
    .await;
    assert!(matches!(result.result, Err(Error::Unavailable)));
    let row: (String, String) = sqlx::query_as(
        "SELECT outcome,error_code FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(result.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, ("failed".into(), "unavailable".into()));
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.accounts WHERE display_name='rollback'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::raw_sql("DROP TRIGGER reject_audit ON management.audit_events; DROP TABLE management.command_invocations; DROP TABLE management.security_events;").execute(&pool).await.unwrap();
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::CreateAccount(NewAccount {
            display_name: "kept",
            role: Role::Reader,
        }),
    )
    .await;
    assert!(matches!(result.result, Ok(Output::Account(_))));
    assert!(matches!(
        service::execute(
            &pool,
            Proof::Token(&admin.token),
            Surface::Cli,
            Command::Accounts(Page::default().into())
        )
        .await
        .result,
        Err(Error::Forbidden)
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM management.audit_events WHERE request_id=$1")
            .bind(result.request_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn commit_failure_is_conservative_unknown_without_retry(pool: PgPool) {
    let admin = owner(&pool).await;
    sqlx::raw_sql("CREATE FUNCTION management.reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-database-detail'; END $$;
        CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON management.audit_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION management.reject_commit();")
        .execute(&pool).await.unwrap();
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::CreateAccount(NewAccount {
            display_name: "unknown",
            role: Role::Reader,
        }),
    )
    .await;
    assert!(matches!(result.result, Err(Error::OutcomeUnknown)));
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT outcome,error_code FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(result.request_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows, vec![("unknown".into(), "outcome_unknown".into())]);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn slow_history_cannot_hold_successful_response_indefinitely(pool: PgPool) {
    let admin = owner(&pool).await;
    let mut fence = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE management.command_invocations IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *fence)
        .await
        .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        service::execute(
            &pool,
            Proof::Session(&admin.session),
            Surface::Console,
            Command::CreateAccount(NewAccount {
                display_name: "bounded",
                role: Role::Reader,
            }),
        ),
    )
    .await
    .unwrap();
    assert!(matches!(result.result, Ok(Output::Account(_))));
    fence.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM management.audit_events WHERE request_id=$1"
        )
        .bind(result.request_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn login_budget_is_shared_bounded_and_resets(pool: PgPool) {
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..80 {
        let pool = pool.clone();
        tasks.spawn(async move { admission::anonymous_allowed(&pool).await.unwrap() });
    }
    let mut admitted = 0;
    while let Some(result) = tasks.join_next().await {
        if result.unwrap() {
            admitted += 1;
        }
    }
    assert_eq!(admitted, 60);
    sqlx::query(
        "UPDATE management.login_budget SET window_start=clock_timestamp()-interval '2 minutes'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(admission::anonymous_allowed(&pool).await.unwrap());
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM management.login_budget WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    sqlx::query("DELETE FROM management.login_budget")
        .execute(&pool)
        .await
        .unwrap();
    assert!(!admission::anonymous_allowed(&pool).await.unwrap());
}
