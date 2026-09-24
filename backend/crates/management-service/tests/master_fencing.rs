#![allow(clippy::unwrap_used)]
mod support;
use filegate_db::{PgPool, management as db};
use grove_management_service::{
    self as service, Error,
    master::{self, Command},
};
use support::*;

#[path = "support/master.rs"]
mod master_support;
use master_support::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_rotation_audit_preserves_the_active_generation(pool: PgPool) {
    let old = config(1, 900);
    old.install(&pool).await.unwrap();
    login(&pool, &old, 900, 901).await;
    reject_audit(&pool).await;
    assert!(matches!(
        config(2, 950).install(&pool).await,
        Err(Error::Unavailable)
    ));
    assert!(
        master::execute(&pool, &old, &hash(901), Command::Current)
            .await
            .result
            .is_ok()
    );
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM management.master_configuration WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(generation, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn generation_fences_old_replicas_and_never_rolls_back(pool: PgPool) {
    let old = config(1, 900);
    assert!(old.install(&pool).await.unwrap());
    let before = audit_count(&pool).await;
    assert!(old.install(&pool).await.unwrap());
    assert_eq!(audit_count(&pool).await, before);
    login(&pool, &old, 900, 901).await;
    let conflicting = config(1, 999);
    assert!(!conflicting.install(&pool).await.unwrap());
    assert!(matches!(
        master::login(&pool, &conflicting, Some(&hash(999)), &hash(902))
            .await
            .result,
        Err(Error::Unavailable)
    ));
    let new = config(2, 950);
    assert!(new.install(&pool).await.unwrap());
    assert!(!old.install(&pool).await.unwrap());
    assert!(matches!(
        master::execute(&pool, &old, &hash(901), Command::Current)
            .await
            .result,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        master::execute(&pool, &new, &hash(901), Command::Current)
            .await
            .result,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        master::login(&pool, &old, Some(&hash(900)), &hash(903))
            .await
            .result,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        master::login(&pool, &new, Some(&hash(900)), &hash(904))
            .await
            .result,
        Err(Error::Unauthenticated)
    ));
    login(&pool, &new, 950, 905).await;
    assert!(
        master::execute(&pool, &new, &hash(905), Command::Current)
            .await
            .result
            .is_ok()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn master_session_expiry_eviction_and_shared_budget_are_bounded(pool: PgPool) {
    let cfg = config(1, 900);
    cfg.install(&pool).await.unwrap();
    for n in 100..109 {
        login(&pool, &cfg, 900, n).await;
    }
    let (count,bounded): (i64,bool) = sqlx::query_as("SELECT count(*),bool_and(expires_at=created_at+interval '10 minutes') FROM management.sessions WHERE auth_method='master' AND revoked_at IS NULL")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 8);
    assert!(bounded);
    assert!(matches!(
        master::execute(&pool, &cfg, &hash(100), Command::Current)
            .await
            .result,
        Err(Error::Unauthenticated)
    ));
    sqlx::query("UPDATE management.sessions SET created_at=clock_timestamp()-interval '20 minutes',expires_at=clock_timestamp()-interval '10 minutes'").execute(&pool).await.unwrap();
    assert!(matches!(
        master::execute(&pool, &cfg, &hash(108), Command::Current)
            .await
            .result,
        Err(Error::Unauthenticated)
    ));
    sqlx::query("UPDATE management.login_budget SET attempts=59")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        master::login(&pool, &cfg, None, &hash(200)).await.result,
        Err(Error::Unauthenticated)
    ));
    assert!(matches!(
        service::sessions::login(&pool, None, &hash(201))
            .await
            .result,
        Err(Error::RateLimited)
    ));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn queued_recovery_rechecks_configuration_after_rotation(pool: PgPool) {
    let owner = owner(&pool).await;
    let cfg = config(1, 900);
    cfg.install(&pool).await.unwrap();
    login(&pool, &cfg, 900, 901).await;
    let mut fence = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(5139268467995599950)")
        .execute(&mut *fence)
        .await
        .unwrap();
    let task_pool = pool.clone();
    let task = tokio::spawn(async move {
        master::execute(
            &task_pool,
            &cfg,
            &hash(901),
            Command::Recover {
                account: owner.account,
                key: key(&hash(2)),
            },
        )
        .await
    });
    wait_for_identity_lock(&pool).await;
    sqlx::query("UPDATE management.master_configuration SET generation=2,token_hash=$1")
        .bind(hash(999))
        .execute(&mut *fence)
        .await
        .unwrap();
    fence.commit().await.unwrap();
    assert!(matches!(
        task.await.unwrap().result,
        Err(Error::Unavailable)
    ));
    assert!(db::authenticate(&pool, &hash(1)).await.unwrap().is_some());
    assert!(db::authenticate(&pool, &hash(2)).await.unwrap().is_none());
}
