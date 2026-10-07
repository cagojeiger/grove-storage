use super::*;
use axum::response::Response;
use grove_db::{PgPool, management as db};
use uuid::Uuid;

async fn request(
    state: &AppState,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    app(state.clone(), &[])
        .oneshot(builder.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn disabled_legacy_routes_never_authenticate_or_access_database() {
    let state = test_state();
    for path in [
        "/api/admin/v1",
        "/api/admin/v1/",
        "/api/admin/v1/session",
        "/api/admin/v1/clients",
        "/api/admin/v1/clients/app/keys",
        "/api/admin/v1/unknown",
    ] {
        for method in ["GET", "POST", "PUT", "DELETE"] {
            let response = request(
                &state,
                method,
                path,
                &[
                    ("authorization", "Bearer test-operator-token"),
                    ("cookie", "__Host-filegate_session=fgss_old"),
                ],
                "{}",
            )
            .await;
            assert_eq!(response.status(), StatusCode::GONE, "{method} {path}");
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                "no-store"
            );
            assert!(body_text(response).await.contains("legacy_admin_removed"));
        }
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn account_console_commands_and_mcp_work_without_legacy_auth(pool: PgPool) {
    let mut state = test_state();
    state.pool = pool;
    state.console_origin = Some("https://console.test".into());
    let password = "a private phrase for the retirement test";
    let account = grove_management_service::local_accounts::initialize(
        &state.pool,
        Uuid::new_v4(),
        "owner",
        "Owner",
        password.into(),
    )
    .await
    .unwrap();
    let token = format!("gsm_{}", grove_core::generate_url_secret());
    db::issue_credential(
        &state.pool,
        &db::AuditContext {
            actor: db::AuditActor::Account {
                id: account,
                credential_id: None,
                session_id: None,
            },
            request_id: Uuid::new_v4(),
            surface: grove_management_policy::Surface::Console,
        },
        account,
        &db::NewCredential {
            label: "test",
            token_prefix: "gsm_test",
            token_hash: &crate::accounts::secrets::token_hash(&token),
            expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        },
    )
    .await
    .unwrap();
    let bearer = format!("Bearer {token}");
    let response = request(
        &state,
        "POST",
        "/api/admin/commands/v1",
        &[("authorization", &bearer)],
        r#"{"protocol":1,"command":"storage.list","input":{}}"#,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = request(
        &state,
        "POST",
        "/api/admin/mcp",
        &[
            ("authorization", &bearer),
            ("accept", "application/json, text/event-stream"),
            ("host", "127.0.0.1"),
            ("mcp-method", "tools/list"),
            ("mcp-protocol-version", "2026-07-28"),
        ],
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = request(
        &state,
        "POST",
        "/api/admin/identity/v1/session",
        &[("origin", "https://console.test"), ("x-grove-csrf", "1")],
        &serde_json::json!({"username":"owner","password":password}).to_string(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let response = request(
        &state,
        "GET",
        "/api/admin/identity/v1/accounts",
        &[("cookie", &cookie)],
        "",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = request(
        &state,
        "POST",
        "/api/admin/console-commands/v1",
        &[
            ("cookie", &cookie),
            ("origin", "https://console.test"),
            ("x-grove-csrf", "1"),
        ],
        r#"{"protocol":1,"command":"storage.list","input":{}}"#,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}
