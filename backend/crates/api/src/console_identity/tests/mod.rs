#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod browser;
mod failures;
mod lifecycle;

use super::{browser::COOKIE, secrets};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use chrono::{Duration, Utc};
use filegate_db::{PgPool, management as db};
use grove_management_policy::{Role, Surface};
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://console.test";
const PATH: &str = "/api/admin/identity/v1/session";

fn app(pool: &PgPool) -> Router {
    let mut state = crate::routes::tests::test_state();
    state.pool = pool.clone();
    state.console_origin = Some(ORIGIN.into());
    crate::routes::app(state, &[])
}

fn context() -> db::AuditContext {
    db::AuditContext {
        actor: db::AuditActor::Master { session_id: None },
        request_id: Uuid::new_v4(),
        surface: Surface::Console,
    }
}

async fn account(pool: &PgPool, role: Role) -> (Uuid, Uuid, String) {
    let initialized: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM management.users)")
        .fetch_one(pool)
        .await
        .unwrap();
    if !initialized {
        db::bootstrap(
            pool,
            &context(),
            "Owner",
            &db::NewCredential {
                label: "bootstrap-fixture",
                token_prefix: "gsm_test",
                token_hash: &secrets::token_hash("fixture-only"),
                expires_at: Utc::now() + Duration::days(1),
            },
        )
        .await
        .unwrap();
    }
    let user = db::create_account(
        pool,
        &context(),
        db::NewAccount::User {
            display_name: "test-user",
            role,
        },
    )
    .await
    .unwrap();
    let (credential, token) = credential(pool, user).await;
    (user, credential, token)
}

async fn credential(pool: &PgPool, user: Uuid) -> (Uuid, String) {
    let raw = format!(
        "{}{}",
        secrets::TOKEN_PREFIX,
        filegate_core::generate_url_secret()
    );
    let credential = db::issue_credential(
        pool,
        &context(),
        user,
        &db::NewCredential {
            label: "test-token",
            token_prefix: "gsm_test",
            token_hash: &secrets::token_hash(&raw),
            expires_at: Utc::now() + Duration::days(1),
        },
    )
    .await
    .unwrap();
    (credential.id, raw)
}

async fn request(
    router: Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: String,
) -> Response {
    let mut builder = Request::builder().method(method).uri(path);
    for (key, value) in headers {
        builder = builder.header(*key, *value);
    }
    let response = router
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    if path == PATH {
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    response
}

async fn login(router: Router, token: &str) -> Response {
    request(
        router,
        "POST",
        PATH,
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"token":token}).to_string(),
    )
    .await
}

fn cookie(response: &Response) -> String {
    response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .into()
}

async fn current(router: Router, cookie: &str) -> Response {
    request(router, "GET", PATH, &[("cookie", cookie)], String::new()).await
}

async fn logout(router: Router, cookie: &str) -> Response {
    request(
        router,
        "DELETE",
        PATH,
        &[
            ("cookie", cookie),
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
        ],
        String::new(),
    )
    .await
}

async fn json(response: Response) -> serde_json::Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 64 * 1024).await.unwrap()).unwrap()
}
