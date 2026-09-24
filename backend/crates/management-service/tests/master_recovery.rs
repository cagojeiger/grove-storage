#![allow(clippy::unwrap_used)]
mod support;
use filegate_db::{PgPool, management as db};
use grove_management_policy::Surface;
use grove_management_service::{
    self as service, Error, Proof,
    master::{self, Command, Output},
};
use support::*;

#[path = "support/master.rs"]
mod master_support;
use master_support::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn bootstrap_is_atomic_single_use_and_concurrent_setup_has_one_winner(pool: PgPool) {
    let cfg = config(1, 900);
    cfg.install(&pool).await.unwrap();
    login(&pool, &cfg, 900, 901).await;
    login(&pool, &cfg, 900, 902).await;
    let a = hash(1);
    let b = hash(2);
    let sa = hash(901);
    let sb = hash(902);
    let (a, b) = tokio::join!(
        master::execute(
            &pool,
            &cfg,
            &sa,
            Command::Bootstrap {
                name: "Owner A",
                key: key(&a)
            }
        ),
        master::execute(
            &pool,
            &cfg,
            &sb,
            Command::Bootstrap {
                name: "Owner B",
                key: key(&b)
            }
        )
    );
    assert_ne!(a.result.is_ok(), b.result.is_ok());
    let (won, lost, session) = if a.result.is_ok() {
        (a, b, sa)
    } else {
        (b, a, sb)
    };
    assert!(matches!(lost.result, Err(Error::Conflict)));
    assert!(matches!(won.result, Ok(Output::Credential(_))));
    assert!(matches!(
        master::execute(
            &pool,
            &cfg,
            &session,
            Command::Bootstrap {
                name: "retry",
                key: key(&hash(3))
            }
        )
        .await
        .result,
        Err(Error::Unauthenticated)
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM management.accounts WHERE role='admin'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM management.audit_events WHERE request_id=$1")
            .bind(won.request_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 3);
    assert!(matches!(
        service::execute(
            &pool,
            Proof::Session(&hash(902)),
            Surface::Console,
            service::Command::CurrentSession
        )
        .await
        .result,
        Err(Error::Unauthenticated)
    ));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn recovery_audit_failure_preserves_old_credentials_and_master_session(pool: PgPool) {
    let owner = owner(&pool).await;
    let cfg = config(1, 900);
    cfg.install(&pool).await.unwrap();
    login(&pool, &cfg, 900, 901).await;
    reject_audit(&pool).await;
    let result = master::execute(
        &pool,
        &cfg,
        &hash(901),
        Command::Recover {
            account: owner.account,
            key: key(&hash(2)),
        },
    )
    .await;
    assert!(matches!(result.result, Err(Error::Unavailable)));
    assert!(
        db::authenticate(&pool, &owner.token)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        db::session_actor(&pool, &owner.session)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        master::execute(&pool, &cfg, &hash(901), Command::Current)
            .await
            .result
            .is_ok()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn recovery_commit_error_is_unknown_and_is_not_retried(pool: PgPool) {
    let owner = owner(&pool).await;
    let cfg = config(1, 900);
    cfg.install(&pool).await.unwrap();
    login(&pool, &cfg, 900, 901).await;
    sqlx::raw_sql("CREATE SEQUENCE management.recovery_attempts;
        CREATE FUNCTION management.reject_recovery() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('management.recovery_attempts'); RAISE EXCEPTION 'private'; END $$;
        CREATE CONSTRAINT TRIGGER reject_recovery AFTER INSERT ON management.credentials DEFERRABLE INITIALLY DEFERRED
        FOR EACH ROW EXECUTE FUNCTION management.reject_recovery();").execute(&pool).await.unwrap();
    let result = master::execute(
        &pool,
        &cfg,
        &hash(901),
        Command::Recover {
            account: owner.account,
            key: key(&hash(2)),
        },
    )
    .await;
    assert!(matches!(result.result, Err(Error::OutcomeUnknown)));
    let tries: i64 = sqlx::query_scalar("SELECT last_value FROM management.recovery_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tries, 1);
    let outcome: String = sqlx::query_scalar(
        "SELECT outcome FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(result.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outcome, "unknown");
    assert!(
        db::authenticate(&pool, &owner.token)
            .await
            .unwrap()
            .is_some()
    );
}
