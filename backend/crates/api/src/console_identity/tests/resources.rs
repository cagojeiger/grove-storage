use super::*;
use serde_json::json;

const COMMANDS: &str = "/api/admin/console-commands/v1";

async fn call(pool: &PgPool, cookie: &str, command: &str, input: serde_json::Value) -> Response {
    request(
        app(pool),
        "POST",
        COMMANDS,
        &[
            ("cookie", cookie),
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        json!({"protocol":1,"command":command,"input":input}).to_string(),
    )
    .await
}

#[sqlx::test(migrations = "../db/migrations")]
async fn resource_session_rechecks_role_and_revocation_with_console_audit(pool: PgPool) {
    let (user, credential, token) = account(&pool, Role::Operator).await;
    crate::resource_commands::tests::seed(&pool).await;
    let cookie = cookie(&login(app(&pool), &token).await);
    let response = call(
        &pool,
        &cookie,
        "client.create",
        json!({"id":"console-app","storage_id":"local"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = json(response).await;
    let request_id: Uuid = body["request_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(body["command"], "client.create");
    let record: (String, Uuid, Uuid) = sqlx::query_as(
        "SELECT surface,actor_id,credential_id FROM management.audit_events WHERE request_id=$1",
    )
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(record, ("console".into(), user, credential));
    db::change_account(
        &pool,
        &context(),
        user,
        db::AccountChange::Role(Role::Viewer),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&pool, &cookie, "storage.list", json!({}))
            .await
            .status(),
        StatusCode::OK
    );
    let denied = call(&pool, &cookie, "client.delete", json!({"id":"console-app"})).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        json(denied).await["error"],
        json!({"code":"forbidden","outcome":"not_applied"})
    );
    db::revoke_credential(&pool, &context(), credential)
        .await
        .unwrap();
    assert_eq!(
        call(&pool, &cookie, "storage.list", json!({}))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn console_commands_reject_other_credentials_and_cross_site_requests(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Admin).await;
    let cookie = cookie(&login(app(&pool), &token).await);
    let authorization = format!("Bearer {token}");
    let body = json!({"protocol":1,"command":"storage.list","input":{}}).to_string();
    for (headers, expected) in [
        (vec![("cookie", cookie.as_str())], StatusCode::FORBIDDEN),
        (
            vec![
                ("cookie", cookie.as_str()),
                ("origin", "https://evil.test"),
                ("x-grove-csrf", "1"),
            ],
            StatusCode::FORBIDDEN,
        ),
        (
            vec![
                ("cookie", cookie.as_str()),
                ("origin", ORIGIN),
                ("x-grove-csrf", "1"),
                ("sec-fetch-site", "cross-site"),
            ],
            StatusCode::FORBIDDEN,
        ),
        (
            vec![
                ("authorization", authorization.as_str()),
                ("origin", ORIGIN),
                ("x-grove-csrf", "1"),
            ],
            StatusCode::UNAUTHORIZED,
        ),
        (
            vec![
                ("cookie", "__Host-filegate_session=legacy"),
                ("origin", ORIGIN),
                ("x-grove-csrf", "1"),
            ],
            StatusCode::UNAUTHORIZED,
        ),
        (
            vec![
                ("cookie", "__Host-grove_setup=setup"),
                ("origin", ORIGIN),
                ("x-grove-csrf", "1"),
            ],
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        let mut headers = headers;
        headers.push(("content-type", "application/json"));
        assert_eq!(
            request(app(&pool), "POST", COMMANDS, &headers, body.clone())
                .await
                .status(),
            expected
        );
    }
    // Session authority does not leak into the machine or legacy transports.
    for path in ["/api/admin/commands/v1", "/api/admin/v1/storages"] {
        let method = if path.ends_with("storages") {
            "GET"
        } else {
            "POST"
        };
        assert_eq!(
            request(
                app(&pool),
                method,
                path,
                &[("cookie", &cookie), ("content-type", "application/json")],
                body.clone()
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        call(&pool, &cookie, "account.list", json!({}))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn console_rejects_filesystem_storage_commands(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Admin).await;
    let cookie = cookie(&login(app(&pool), &token).await);
    for name in ["storage.create", "storage.replace"] {
        let response = call(&pool, &cookie, name,
            json!({"id":"local","spec":{"kind":"fs","root_path":"/never-probed","capacity_bytes":1}})).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(json(response).await["error"]["code"], "invalid_input");
    }
    assert!(
        filegate_db::registry::list_storages(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}
