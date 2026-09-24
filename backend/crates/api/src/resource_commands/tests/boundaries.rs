use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn cookies_legacy_master_and_ambiguous_authorization_cannot_enter_resource_transport(
    pool: PgPool,
) {
    let token = owner(&pool).await;
    let bearer = format!("Bearer {token}");
    let body = json!({"protocol":1,"command":"client.list","input":{}});
    for headers in [
        vec![],
        vec![("authorization", "Bearer test-operator-token")],
        vec![("authorization", "Bearer gsmt_invalid")],
        vec![
            ("authorization", bearer.as_str()),
            ("authorization", bearer.as_str()),
        ],
        vec![
            ("authorization", bearer.as_str()),
            ("cookie", "__Host-grove_session=ignored"),
        ],
        vec![("cookie", "__Host-grove_session=ignored")],
        vec![("x-auth-request-user", "Owner")],
    ] {
        let response = request(&pool, "POST", PATH, &headers, body.clone()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::LOCATION));
        assert_eq!(json_body(response).await["error"]["outcome"], "not_applied");
    }
    let response = request(
        &pool,
        "GET",
        "/api/admin/identity/v1/accounts",
        &[("authorization", &bearer)],
        Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = request(
        &pool,
        "GET",
        "/api/admin/v1/clients",
        &[("authorization", &bearer)],
        Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn envelope_validation_and_server_owned_surface_are_enforced(pool: PgPool) {
    let token = owner(&pool).await;
    for (body, code) in [
        (
            json!({"protocol":2,"command":"invalid","input":{}}),
            "protocol_incompatible",
        ),
        (
            json!({"protocol":1,"command":"identity.account.create","input":{}}),
            "unknown_command",
        ),
        (
            json!({"protocol":1,"command":"client.list","input":{},"surface":"console"}),
            "invalid_input",
        ),
        (
            json!({"protocol":1,"command":"client.list","input":{"actor_id":"forged"}}),
            "invalid_input",
        ),
        (json!([1, "client.list", {}]), "invalid_input"),
        (
            json!({"protocol":1,"command":"usage.history","input":{"days":0}}),
            "invalid_input",
        ),
    ] {
        let response = request(
            &pool,
            "POST",
            PATH,
            &[("authorization", &format!("Bearer {token}"))],
            body,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json_body(response).await["error"]["code"], code);
    }
    let response = request(
        &pool,
        "POST",
        PATH,
        &[
            ("authorization", &format!("Bearer {token}")),
            ("user-agent", "cli-mcp-console"),
            ("x-request-id", "client-spoof"),
        ],
        json!({"protocol":1,"command":"client.list","input":{}}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    let id: Uuid = body["request_id"].as_str().unwrap().parse().unwrap();
    let surface: String = sqlx::query_scalar(
        "SELECT surface FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(surface, "resource_api");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn live_role_and_token_revocation_affect_following_http_reads(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    let identity = db::authenticate(&pool, &secrets::token_hash(&token))
        .await
        .unwrap()
        .unwrap();
    // A second Admin keeps the last-Admin invariant while testing demotion.
    let context = db::AuditContext {
        actor: db::AuditActor::Master { session_id: None },
        request_id: Uuid::new_v4(),
        surface: Surface::Console,
    };
    db::create_account(
        &pool,
        &context,
        db::NewAccount {
            display_name: "Other",
            role: Role::Admin,
        },
    )
    .await
    .unwrap();
    db::change_account(
        &pool,
        &context,
        identity.account_id,
        db::AccountChange::Role(Role::Reader),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&pool, &token, "client.list", json!({})).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        call(&pool, &token, "credential.list", json!({"client_id":"app"}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    db::revoke_credential(&pool, &context, identity.credential_id)
        .await
        .unwrap();
    assert_eq!(
        call(&pool, &token, "client.list", json!({})).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
