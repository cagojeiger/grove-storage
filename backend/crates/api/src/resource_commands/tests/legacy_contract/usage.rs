use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn usage_overflow_returns_errors_instead_of_panicking_or_wrapping(pool: PgPool) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql(
        "INSERT INTO clients(id,storage_id) VALUES('large','vendor');
        WITH f AS (INSERT INTO files(client_id,state,declared_size,committed_at)
            VALUES('large','pending',9223372036854775807,NULL),
                  ('large','active',9223372036854775807,now()) RETURNING id)
        INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'vendor',id::text FROM f;",
    )
    .execute(&pool)
    .await
    .unwrap();
    let new = payload(
        api.command("usage.storages", json!({})).await,
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    assert_eq!(
        new["error"],
        json!({"code":"unavailable","outcome":"not_applied"})
    );
    let old = payload(
        api.legacy("GET", "/usage", Value::Null).await,
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await;
    assert_eq!(old, json!({"error":"internal error"}));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn history_outputs_match_while_each_transport_keeps_its_input_policy(pool: PgPool) {
    let api = Pair::new(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql(
        "INSERT INTO usage_snapshot(day,storage_id,client_id,active_bytes,active_files)
        VALUES(current_date-2,'local','app',20,2),(current_date-100,'local','app',100,10)",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (days, count) in [(1, 1), (90, 2), (3650, 3)] {
        let rows = api
            .same_read(
                &format!("/usage/history?days={days}"),
                "usage.history",
                json!({"days":days}),
            )
            .await;
        assert_eq!(rows.as_array().unwrap().len(), count);
    }
    // REST clamps numeric days; the command contract rejects out-of-range values.
    for (days, count) in [(0, 1), (3651, 3)] {
        let rows = payload(
            api.legacy("GET", &format!("/usage/history?days={days}"), Value::Null)
                .await,
            StatusCode::OK,
        )
        .await;
        assert_eq!(rows.as_array().unwrap().len(), count);
        let error = payload(
            api.command("usage.history", json!({"days":days})).await,
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_eq!(
            error["error"],
            json!({"code":"invalid_input","outcome":"not_applied"})
        );
    }
}
