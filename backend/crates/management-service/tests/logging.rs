#![allow(clippy::unwrap_used)]
#[path = "../../db/tests/support/time.rs"]
mod clock;
mod support;
use grove_db::{
    PgPool,
    management::{self as db, NewAccount, admission},
};
use grove_management_policy::{Role, Surface};
use grove_management_service::{self as service, Command, Error, Output, Page, Proof};
use support::*;

fn details() -> db::telemetry::Details {
    db::telemetry::Details::new(
        "identity.session.login",
        db::telemetry::Stage::Authentication,
    )
}

#[sqlx::test(migrations = "../db/migrations")]
async fn mutations_audit_once_and_successful_reads_do_not_grow_history(pool: PgPool) {
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
    for command in [
        Command::Accounts(Page::default().into()),
        Command::Account(admin.account),
        Command::CurrentSession,
        Command::OwnSessions(Page::default()),
        Command::Audit(Page::default().into()),
        Command::Invocations(Page::default().into()),
        Command::Security(Page::default().into()),
    ] {
        let execution = service::execute(
            &pool,
            Proof::Session(&admin.session),
            Surface::Console,
            command,
        )
        .await;
        assert!(execution.result.is_ok());
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM management.command_invocations WHERE request_id=$1",
        )
        .bind(execution.request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }
    let row: (String, String, String) = sqlx::query_as("SELECT i.target_type,i.target_id,a.resource_id FROM management.command_invocations i JOIN management.audit_events a USING(request_id) WHERE i.request_id=$1")
        .bind(created.request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, "account");
    assert_eq!(row.1, row.2);
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
    let fields: serde_json::Value = serde_json::from_str(&event).unwrap();
    assert_eq!(fields.get("operation").unwrap(), "identity.account.list");
    assert_eq!(fields.get("stage").unwrap(), "authentication");
    assert_eq!(fields.get("auth_method").unwrap(), "api_token");
    assert_eq!(fields.get("is_sampled").unwrap(), true);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn anonymous_security_log_growth_is_bounded_across_concurrent_callers(pool: PgPool) {
    use db::telemetry::{self, SecurityReason};
    clock::install_clock(&pool).await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..80 {
        let pool = pool.clone();
        tasks.spawn(async move {
            telemetry::security(
                &pool,
                None,
                uuid::Uuid::new_v4(),
                Surface::Mcp,
                SecurityReason::Unauthenticated,
                &details(),
            )
            .await
            .unwrap();
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.security_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    for (surface, reason) in [
        (Surface::Cli, SecurityReason::Unauthenticated),
        (Surface::Mcp, SecurityReason::Forbidden),
    ] {
        telemetry::security(
            &pool,
            None,
            uuid::Uuid::new_v4(),
            surface,
            reason,
            &details(),
        )
        .await
        .unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.security_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
    clock::set_time(
        &pool,
        clock::base() + chrono::Duration::seconds(60) - chrono::Duration::nanoseconds(1),
    )
    .await;
    telemetry::security(
        &pool,
        None,
        uuid::Uuid::new_v4(),
        Surface::Mcp,
        SecurityReason::Unauthenticated,
        &details(),
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.security_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
    clock::set_time(&pool, clock::base() + chrono::Duration::seconds(60)).await;
    telemetry::security(
        &pool,
        None,
        uuid::Uuid::new_v4(),
        Surface::Mcp,
        SecurityReason::Unauthenticated,
        &details(),
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.security_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 4);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn authenticated_security_events_are_not_sampled(pool: PgPool) {
    use db::telemetry::{self, SecurityReason};
    let admin = owner(&pool).await;
    for _ in 0..5 {
        let context = db::AuditContext {
            actor: db::AuditActor::Account {
                id: admin.account,
                credential_id: Some(admin.credential),
                session_id: Some(admin.session_id),
            },
            request_id: uuid::Uuid::new_v4(),
            surface: Surface::Console,
        };
        telemetry::security(
            &pool,
            Some(&context),
            context.request_id,
            Surface::Console,
            SecurityReason::Forbidden,
            &details(),
        )
        .await
        .unwrap();
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.security_events WHERE actor_kind='account'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 5);
    let sampled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.security_events WHERE is_sampled IS DISTINCT FROM false",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sampled, 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn anonymous_logger_does_not_wait_on_another_logger(pool: PgPool) {
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(4674380, hashtext('mcp:unauthenticated'))")
        .execute(&mut *lock)
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        db::telemetry::security(
            &pool,
            None,
            uuid::Uuid::new_v4(),
            Surface::Mcp,
            db::telemetry::SecurityReason::Unauthenticated,
            &details(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.security_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    lock.rollback().await.unwrap();
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
    let stages: (String, String) = sqlx::query_as("SELECT i.stage,s.stage FROM management.command_invocations i JOIN management.security_events s USING(request_id) WHERE i.request_id=$1")
        .bind(denied.request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(stages, ("authorization".into(), "authorization".into()));
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
    let stage: String =
        sqlx::query_scalar("SELECT stage FROM management.command_invocations WHERE request_id=$1")
            .bind(result.request_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stage, "commit");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_reads_keep_the_target_and_sensitive_reads_keep_success(pool: PgPool) {
    let admin = owner(&pool).await;
    let missing = uuid::Uuid::new_v4();
    let failed = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Account(missing),
    )
    .await;
    assert!(matches!(failed.result, Err(Error::NotFound)));
    let row: (String, String, String, String) = sqlx::query_as("SELECT target_type,target_id,stage,outcome FROM management.command_invocations WHERE request_id=$1")
        .bind(failed.request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        row,
        (
            "account".into(),
            missing.to_string(),
            "execution".into(),
            "failed".into()
        )
    );
    let listed = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Credentials {
            account: admin.account,
            page: Page::default(),
        },
    )
    .await;
    assert!(listed.result.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations WHERE request_id=$1 AND operation='identity.credential.list' AND outcome='succeeded'")
        .bind(listed.request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn browser_admission_is_distinct_from_account_authorization(pool: PgPool) {
    let id = service::sessions::reject_browser(&pool, Error::Forbidden).await;
    let row: (String, String, String, bool) = sqlx::query_as("SELECT operation,stage,auth_method,is_sampled FROM management.security_events WHERE request_id=$1")
        .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        row,
        (
            "console.request".into(),
            "browser_admission".into(),
            "session".into(),
            true
        )
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn password_login_and_reauthentication_keep_context_not_secrets(pool: PgPool) {
    let password = "a private phrase for collection contract tests";
    service::local_accounts::initialize(
        &pool,
        uuid::Uuid::new_v4(),
        "owner",
        "Private owner name",
        password.into(),
    )
    .await
    .unwrap();
    let failed = service::sessions::login_password(
        &pool,
        "private-unknown-user",
        "private-invalid-password".into(),
        &hash(900),
    )
    .await;
    assert!(matches!(failed.result, Err(Error::Unauthenticated)));
    let session_hash = hash(901);
    let login =
        service::sessions::login_password(&pool, "owner", password.into(), &session_hash).await;
    assert!(login.result.is_ok());
    let row: (String, String, bool) = sqlx::query_as(
        "SELECT stage,auth_method,is_sampled FROM management.security_events WHERE request_id=$1",
    )
    .bind(login.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, ("authentication".into(), "password".into(), false));
    for correct in [false, true] {
        let id = uuid::Uuid::new_v4();
        let result = service::password_setups::create(
            &pool,
            id,
            &session_hash,
            NewAccount {
                display_name: "Private reader name",
                role: Role::Reader,
            },
            "reader",
            if correct {
                password
            } else {
                "private-wrong-password"
            }
            .into(),
            &hash(902),
        )
        .await;
        assert_eq!(result.is_ok(), correct);
        let row: (String, String, String, bool) = sqlx::query_as("SELECT operation,stage,auth_method,is_sampled FROM management.security_events WHERE request_id=$1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(
            row,
            (
                "identity.account.create".into(),
                "reauthentication".into(),
                "password".into(),
                false
            )
        );
    }
    let histories: String = sqlx::query_scalar("SELECT jsonb_build_object('audit',(SELECT jsonb_agg(to_jsonb(a)) FROM management.audit_events a),'commands',(SELECT jsonb_agg(to_jsonb(i)) FROM management.command_invocations i),'security',(SELECT jsonb_agg(to_jsonb(s)) FROM management.security_events s))::text")
        .fetch_one(&pool).await.unwrap();
    for secret in [
        password,
        "private-unknown-user",
        "private-invalid-password",
        "private-wrong-password",
        "Private owner name",
        "Private reader name",
        "argon2",
        &session_hash,
    ] {
        assert!(!histories.contains(secret));
    }
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
