use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn discovery_requires_current_bearer_and_rejects_browser_proofs(pool: PgPool) {
    let token = owner(&pool).await;
    let bearer = format!("Bearer {token}");
    for headers in [
        vec![],
        vec![("authorization", "Bearer test-operator-token".into())],
        vec![("authorization", format!("Bearer gsmt_{}", "a".repeat(64)))],
        vec![
            ("authorization", bearer.clone()),
            ("authorization", bearer.clone()),
        ],
        vec![
            ("authorization", bearer.clone()),
            ("cookie", "__Host-grove_session=x".into()),
        ],
        vec![("x-auth-request-user", "Owner".into())],
    ] {
        let response = request(&pool, "POST", &headers, json!({})).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::LOCATION));
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    for origin in ["https://evil.test", "null", "https://console.test"] {
        let response = request(
            &pool,
            "POST",
            &[("authorization", bearer.clone()), ("origin", origin.into())],
            json!({}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    for method in ["GET", "DELETE"] {
        assert_eq!(
            request(
                &pool,
                method,
                &[("authorization", bearer.clone())],
                json!({})
            )
            .await
            .status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }
    let identity = db::authenticate(&pool, &secrets::token_hash(&token))
        .await
        .unwrap()
        .unwrap();
    db::revoke_credential(&pool, &context(), identity.credential_id.unwrap())
        .await
        .unwrap();
    assert_eq!(
        rpc(&pool, &token, "tools/list", json!({})).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn identity_tools_and_forged_inputs_never_reach_executor(pool: PgPool) {
    let token = owner(&pool).await;
    for (name, input, code) in [
        ("identity.account.create", json!({}), "unknown_command"),
        ("history.audit", json!({}), "unknown_command"),
        (
            "client.list",
            json!({"surface":"console","actor_id":"spoof"}),
            "invalid_input",
        ),
        ("usage.history", json!({"days":0}), "invalid_input"),
    ] {
        let response = rpc(
            &pool,
            &token,
            "tools/call",
            json!({"name":name,"arguments":input}),
        )
        .await;
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], -32602);
        assert_eq!(
            body["error"]["data"],
            json!({"code":code,"outcome":"not_applied"})
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
