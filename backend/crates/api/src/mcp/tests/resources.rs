use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn ten_reads_equal_command_http_and_record_one_mcp_invocation(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    for (name, input) in [
        ("status", json!({})),
        ("storage.list", json!({})),
        ("storage.show", json!({"id":"local"})),
        ("client.list", json!({})),
        ("client.show", json!({"id":"app"})),
        ("storage.metadata.show", json!({"id":"local"})),
        ("client.metadata.show", json!({"id":"app"})),
        ("client-key.list", json!({"client_id":"app"})),
        ("credential.list", json!({"client_id":"app"})),
        ("usage.storages", json!({})),
        ("usage.clients", json!({})),
        ("usage.history", json!({"days":7})),
    ] {
        let reply = call(&pool, &token, name, input.clone()).await;
        assert_eq!(reply["isError"], false);
        let output = &reply["structuredContent"];
        let request_id: Uuid = output["request_id"].as_str().unwrap().parse().unwrap();
        let records: Vec<(String, String)> = sqlx::query_as(
            "SELECT surface,operation FROM management.command_invocations WHERE request_id=$1",
        )
        .bind(request_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(records, vec![("mcp".into(), name.into())]);
        let mut state = crate::routes::tests::test_state();
        state.pool = pool.clone();
        let rest = crate::routes::app(state, &[])
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/admin/commands/v1")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"protocol":1,"command":name,"input":input}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(output["result"], json_body(rest).await["result"]);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn metadata_mcp_and_http_share_writes_and_validation(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    for (resource, id) in [("storage", "local"), ("client", "app")] {
        let name = format!("{resource}.metadata.replace");
        let labels = json!({"description":"mcp resource", "environment":"home"});
        let reply = call(&pool, &token, &name, json!({"id":id,"metadata":labels})).await;
        assert_eq!(reply["isError"], false, "{reply}");
        assert_eq!(
            reply["structuredContent"]["result"],
            json!({"id":id,"metadata":labels})
        );
        let mut state = crate::routes::tests::test_state();
        state.pool = pool.clone();
        let response = crate::routes::app(state, &[]).oneshot(
            HttpRequest::builder().method("POST").uri("/api/admin/commands/v1")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"protocol":1,"command":format!("{resource}.metadata.show"),"input":{"id":id}}).to_string())).unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["result"]["metadata"], labels);
        let bad = rpc(
            &pool,
            &token,
            "tools/call",
            json!({"name":name,"arguments":{"id":id,"metadata":{"nested":{}}}}),
        )
        .await;
        assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn user_demotion_limits_writes_and_revocation_stops_discovery(pool: PgPool) {
    let admin_token = owner(&pool).await;
    let admin = db::authenticate(&pool, &secrets::token_hash(&admin_token))
        .await
        .unwrap()
        .unwrap();
    let ctx = context();
    let user = db::create_account(
        &pool,
        &ctx,
        db::NewAccount {
            display_name: "Writer",
            role: Role::Writer,
        },
    )
    .await
    .unwrap();
    let token = format!("gsm_{}", filegate_core::generate_url_secret());
    db::issue_credential(
        &pool,
        &ctx,
        user,
        &db::NewCredential {
            label: "test",
            token_prefix: "gsm_test",
            token_hash: &secrets::token_hash(&token),
            expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        },
    )
    .await
    .unwrap();
    seed(&pool).await;
    let success = call(
        &pool,
        &token,
        "client.create",
        json!({"id":"automation-app","storage_id":"local"}),
    )
    .await;
    assert_eq!(success["isError"], false);
    let id: Uuid = success["structuredContent"]["request_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let row: (String, Uuid, Uuid) = sqlx::query_as(
        "SELECT surface,actor_id,credential_id FROM management.audit_events WHERE request_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let identity = db::authenticate(&pool, &secrets::token_hash(&token))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row, ("mcp".into(), user, identity.credential_id));
    db::change_account(&pool, &ctx, user, db::AccountChange::Role(Role::Reader))
        .await
        .unwrap();
    assert_eq!(
        call(&pool, &token, "client.list", json!({})).await["isError"],
        false
    );
    let denied = call(
        &pool,
        &token,
        "client.create",
        json!({"id":"blocked","storage_id":"local"}),
    )
    .await;
    assert_eq!(denied["isError"], true);
    assert_eq!(
        denied["structuredContent"]["error"],
        json!({"code":"forbidden","outcome":"not_applied"})
    );
    assert!(
        db::authenticate(&pool, &secrets::token_hash(&admin_token))
            .await
            .unwrap()
            .is_some()
    );
    assert_ne!(user, admin.account_id);
    db::change_account(&pool, &ctx, user, db::AccountChange::Active(false))
        .await
        .unwrap();
    assert_eq!(
        rpc(&pool, &token, "tools/list", json!({})).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn mcp_rejects_filesystem_storage_commands(pool: PgPool) {
    let token = owner(&pool).await;
    for name in ["storage.create", "storage.replace"] {
        let response = rpc(&pool, &token, "tools/call",
            json!({"name":name,"arguments":{"id":"local","spec":{"kind":"fs","root_path":"/never-probed","capacity_bytes":1}}})).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = json_body(response).await;
        assert_eq!(body["error"]["code"], -32602);
        assert_eq!(
            body["error"]["data"],
            json!({"code":"invalid_input","outcome":"not_applied"})
        );
    }
    assert!(
        filegate_db::registry::list_storages(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}
