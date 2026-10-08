#![allow(clippy::unwrap_used)]
mod support {
    #[allow(clippy::panic)]
    pub mod lifecycle;
}

use grove_db::PgPool;
use support::lifecycle;

#[sqlx::test(migrations = "./migrations")]
async fn logical_key_cannot_reference_another_clients_file(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    grove_db::registry::insert_client(&pool, "other", "s")
        .await
        .unwrap();
    let file = lifecycle::create_ok(&pool, 10).await;
    let error =
        sqlx::query("INSERT INTO s3_object_keys(client_id,key,file_id) VALUES('other','key',$1)")
            .bind(file.file_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    sqlx::query("INSERT INTO s3_object_keys(client_id,key,file_id) VALUES('c','key',$1)")
        .bind(file.file_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn only_one_issued_write_lease_per_file_but_read_grants_are_independent(pool: PgPool) {
    lifecycle::wire(&pool, 1000).await;
    let file = lifecycle::create_ok(&pool, 10).await;
    let error = sqlx::query(
        "INSERT INTO leases(file_id,kind,expires_at) VALUES($1,'write',grove_time.transaction_now()+interval '1 hour')",
    )
    .bind(file.file_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    for _ in 0..2 {
        sqlx::query("INSERT INTO leases(file_id,kind,expires_at) VALUES($1,'read',grove_time.transaction_now()+interval '1 hour')")
            .bind(file.file_id).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE leases SET state='expired' WHERE id=$1")
        .bind(file.lease_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO leases(file_id,kind,expires_at) VALUES($1,'write',grove_time.transaction_now()+interval '1 hour')")
        .bind(file.file_id).execute(&pool).await.unwrap();
}
