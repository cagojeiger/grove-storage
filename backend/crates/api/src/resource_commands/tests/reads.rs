use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn all_resource_reads_match_legacy_payloads_without_provider_secrets(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    for (command, input, legacy) in [
        ("storage.list", json!({}), "/storages"),
        ("storage.show", json!({"id":"vendor"}), "/storages/vendor"),
        ("client.list", json!({}), "/clients"),
        ("client.show", json!({"id":"app"}), "/clients/app"),
        (
            "client-key.list",
            json!({"client_id":"app"}),
            "/clients/app/keys",
        ),
        (
            "credential.list",
            json!({"client_id":"app"}),
            "/clients/app/s3-credentials",
        ),
        ("usage.storages", json!({}), "/usage"),
        ("usage.clients", json!({}), "/usage/clients"),
        ("usage.history", json!({}), "/usage/history"),
    ] {
        let response = call(&pool, &token, command, input).await;
        assert_eq!(response.status(), StatusCode::OK, "{command}");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        let request_id = response.headers()["x-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let result = json_body(response).await;
        assert_eq!(result["request_id"], request_id);
        assert_eq!(result["command"], command);
        let legacy_response = request(
            &pool,
            "GET",
            &format!("/api/admin/v1{legacy}"),
            &[("authorization", "Bearer test-operator-token")],
            Value::Null,
        )
        .await;
        assert_eq!(legacy_response.status(), StatusCode::OK);
        assert_eq!(
            result["result"],
            json_body(legacy_response).await,
            "{command}"
        );
        let output = result.to_string();
        for forbidden in [
            "private-ciphertext",
            "private-service-cipher",
            "private-key-id",
            "secret_key_ciphertext",
            "secret_key_nonce",
            "enc_key_id",
        ] {
            assert!(!output.contains(forbidden), "{command}: {forbidden}");
        }
    }
    let result = json_body(call(&pool, &token, "status", json!({})).await).await;
    assert_eq!(result["result"]["registry"]["storage_count"], 2);
    assert_eq!(result["result"]["registry"]["client_count"], 1);
    assert_eq!(result["result"]["storage_access"], "not_checked");
    let usage = json_body(call(&pool, &token, "usage.storages", json!({})).await).await;
    assert_eq!(usage["result"][0]["remaining_bytes"], -50);
    assert_eq!(usage["result"][0]["reserved_bytes"], 10);
    assert_eq!(usage["result"][0]["active_bytes"], 120);
    assert_eq!(usage["result"][0]["purge_pending_bytes"], 20);
    for table in ["command_invocations", "security_events"] {
        let rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT row_to_json(t)::text FROM management.{table} t"
        ))
        .fetch_all(&pool)
        .await
        .unwrap();
        for row in rows {
            assert!(!row.contains(&token));
            assert!(!row.contains(&secrets::token_hash(&token)));
        }
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn missing_resource_and_known_mutation_have_stable_errors(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    for (name, input) in [
        ("storage.show", json!({"id":"missing"})),
        ("client.show", json!({"id":"missing"})),
        ("client-key.list", json!({"client_id":"missing"})),
        ("credential.list", json!({"client_id":"missing"})),
    ] {
        let response = call(&pool, &token, name, input).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            json_body(response).await["error"],
            json!({"code":"not_found","outcome":"not_applied"})
        );
    }
    let response = call(&pool, &token, "client.delete", json!({"id":"app"})).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await["error"],
        json!({"code":"request_rejected","outcome":"not_applied"})
    );
    assert!(
        filegate_db::registry::client_exists(&pool, "app")
            .await
            .unwrap()
    );
}
