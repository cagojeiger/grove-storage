#![allow(clippy::unwrap_used)]
mod support;

use filegate_db::{
    PgPool,
    management::admission::{self, Purpose},
};
use support::owner;

#[sqlx::test(migrations = "../db/migrations")]
async fn account_budget_is_atomic_isolated_and_resets(pool: PgPool) {
    let admin = owner(&pool).await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..80 {
        let pool = pool.clone();
        tasks.spawn(async move {
            admission::account_allowed(&pool, admin.account, Purpose::Login)
                .await
                .unwrap()
        });
    }
    let mut admitted = 0;
    while let Some(result) = tasks.join_next().await {
        admitted += usize::from(result.unwrap());
    }
    assert_eq!(admitted, 60);
    assert!(
        admission::account_allowed(&pool, admin.account, Purpose::Reauthentication)
            .await
            .unwrap()
    );
    assert!(admission::anonymous_allowed(&pool).await.unwrap());
    sqlx::query("UPDATE management.authentication_budgets SET window_start=clock_timestamp()-interval '2 minutes' WHERE purpose='login'")
        .execute(&pool).await.unwrap();
    assert!(
        admission::account_allowed(&pool, admin.account, Purpose::Login)
            .await
            .unwrap()
    );
    let attempts: Vec<i32> =
        sqlx::query_scalar("SELECT attempts FROM management.authentication_budgets")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, vec![1, 1]);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn unknown_accounts_create_no_budget_and_database_failure_is_not_admitted(pool: PgPool) {
    for _ in 0..80 {
        assert!(
            !admission::account_allowed(&pool, uuid::Uuid::new_v4(), Purpose::Login)
                .await
                .unwrap()
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.authentication_budgets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let admin = owner(&pool).await;
    sqlx::query("DROP TABLE management.authentication_budgets")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        admission::account_allowed(&pool, admin.account, Purpose::Login)
            .await
            .is_err()
    );
}
