use super::*;

async fn success(pool: &PgPool, token: &str, name: &str, input: Value) -> Value {
    let response = call(pool, token, name, input).await;
    assert_eq!(response.status(), StatusCode::OK, "{name}");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = json_body(response).await;
    let request_id: Uuid = body["request_id"].as_str().unwrap().parse().unwrap();
    let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM management.audit_events WHERE request_id=$1),(SELECT count(*) FROM management.command_invocations WHERE request_id=$1)")
        .bind(request_id).fetch_one(pool).await.unwrap();
    assert_eq!(counts, (1, 1), "{name}");
    body["result"].clone()
}

#[sqlx::test(migrations = "../db/migrations")]
async fn six_resource_writes_round_trip_through_http_and_legacy_reads(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    let result = success(
        &pool,
        &token,
        "client.create",
        json!({"id":"managed","storage_id":"local"}),
    )
    .await;
    assert_eq!(result, json!({"id":"managed","storage_id":"local"}));
    let raw_key = "runtime-secret-for-resource-test";
    let hash = filegate_core::client_key_hash(raw_key);
    let result = success(
        &pool,
        &token,
        "client-key.register",
        json!({"client_id":"managed","key_hash":hash}),
    )
    .await;
    assert_eq!(result, json!({"client_id":"managed","key_hash":hash}));
    let result = success(
        &pool,
        &token,
        "credential.create",
        json!({"client_id":"managed"}),
    )
    .await;
    let access = result["access_key_id"].as_str().unwrap();
    let secret = result["secret_key"].as_str().unwrap();
    let legacy = request(
        &pool,
        "GET",
        "/api/admin/v1/clients/managed/s3-credentials",
        &[("authorization", "Bearer test-operator-token")],
        Value::Null,
    )
    .await;
    assert_eq!(legacy.status(), StatusCode::OK);
    assert_eq!(json_body(legacy).await, json!([access]));
    assert_eq!(
        filegate_db::registry::client_id_for_key_hash(&pool, &hash)
            .await
            .unwrap()
            .as_deref(),
        Some("managed")
    );
    for table in ["audit_events", "command_invocations", "security_events"] {
        let logs: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT row_to_json(t)::text FROM management.{table} t"
        ))
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(logs.iter().all(|line| !line.contains(secret)
            && !line.contains(raw_key)
            && !line.contains(&hash)
            && !line.contains(&token)));
    }
    success(
        &pool,
        &token,
        "credential.delete",
        json!({"client_id":"managed","access_key_id":access}),
    )
    .await;
    success(
        &pool,
        &token,
        "client-key.delete",
        json!({"client_id":"managed","key_hash":hash}),
    )
    .await;
    success(&pool, &token, "client.delete", json!({"id":"managed"})).await;
    assert!(
        filegate_db::s3_registry::get_credential(&pool, access)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        filegate_db::registry::client_id_for_key_hash(&pool, &hash)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !filegate_db::registry::client_exists(&pool, "managed")
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn resource_write_http_keeps_conflicts_and_missing_reference_validation(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    for (command, input, status, code) in [
        (
            "client.delete",
            json!({"id":"app"}),
            StatusCode::CONFLICT,
            "conflict",
        ),
        (
            "client.create",
            json!({"id":"app","storage_id":"local"}),
            StatusCode::CONFLICT,
            "conflict",
        ),
        (
            "client.create",
            json!({"id":"api","storage_id":"local"}),
            StatusCode::BAD_REQUEST,
            "invalid_input",
        ),
        (
            "client.create",
            json!({"id":"new","storage_id":"missing"}),
            StatusCode::BAD_REQUEST,
            "invalid_input",
        ),
        (
            "credential.create",
            json!({"client_id":"missing"}),
            StatusCode::BAD_REQUEST,
            "invalid_input",
        ),
    ] {
        let response = call(&pool, &token, command, input).await;
        assert_eq!(response.status(), status);
        assert_eq!(
            json_body(response).await["error"],
            json!({"code":code,"outcome":"not_applied"})
        );
    }
    assert!(
        filegate_db::registry::client_exists(&pool, "app")
            .await
            .unwrap()
    );
}
