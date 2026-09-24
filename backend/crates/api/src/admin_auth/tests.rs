#![allow(clippy::unwrap_used)]

use axum::{Router, body::Body, http::Request};
use filegate_db::PgPool;
use tower::ServiceExt;

use super::*;

const ORIGIN: &str = "https://console.test";

fn app(pool: PgPool) -> Router {
    let mut state = crate::routes::tests::test_state();
    state.pool = pool;
    state.console_origin = Some(ORIGIN.to_owned());
    crate::routes::app(state, &[])
}

async fn init(pool: &PgPool) -> (String, Uuid) {
    let token = format!("fgop_{}", filegate_core::generate_url_secret());
    let credential = db::issue(
        pool,
        db::IssueMode::Initialize,
        "test",
        &hash("admin-token", &token),
    )
    .await
    .unwrap()
    .unwrap();
    (token, credential.id)
}

async fn login(app: Router, token: &str) -> Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/admin/v1/session")
            .header("origin", ORIGIN)
            .header("x-filegate-csrf", "1")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({"token":token}).to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get(app: Router, path: &str, key: &str, value: &str) -> Response {
    app.oneshot(
        Request::builder()
            .uri(path)
            .header(key, value)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

#[test]
fn credential_domains_and_origin_validation() {
    assert_ne!(hash("admin-token", "same"), hash("admin-session", "same"));
    assert!(console_origin(None).unwrap().is_none());
    assert!(console_origin(Some(ORIGIN.to_owned())).is_ok());
    for origin in [
        "http://console.test",
        "https://console.test/",
        "https://console.test/path",
        "https://user@console.test",
        "https://console.test?x",
        "https://console.test#x",
        "null",
    ] {
        assert!(console_origin(Some(origin.to_owned())).is_err(), "{origin}");
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn token_session_revocation_and_legacy_cutover(pool: PgPool) {
    let router = app(pool.clone());
    let path = "/api/admin/v1/clients";
    assert_eq!(
        get(
            router.clone(),
            path,
            "authorization",
            "Bearer test-operator-token"
        )
        .await
        .status(),
        StatusCode::OK
    );
    let (token, id) = init(&pool).await;
    assert_eq!(
        get(
            router.clone(),
            path,
            "authorization",
            "Bearer test-operator-token"
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(
            router.clone(),
            path,
            "authorization",
            &format!("Bearer {token}")
        )
        .await
        .status(),
        StatusCode::OK
    );
    let response = login(router.clone(), &token).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    for flag in [
        "Secure",
        "HttpOnly",
        "SameSite=Strict",
        "Path=/",
        "Max-Age=",
    ] {
        assert!(cookie.contains(flag));
    }
    let cookie = cookie.split(';').next().unwrap();
    assert_eq!(
        get(router.clone(), path, "cookie", cookie).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        get(router.clone(), "/api/admin/v1/session", "cookie", cookie)
            .await
            .status(),
        StatusCode::OK
    );
    // The service surface never accepts an administrator session or token.
    assert_eq!(
        get(router.clone(), "/api/v1/files", "cookie", cookie)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(db::revoke(&pool, id).await.unwrap());
    assert_eq!(
        get(router.clone(), path, "cookie", cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(router.clone(), &token).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(router, path, "authorization", &format!("Bearer {token}"))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn cookie_mutations_require_csrf_and_logout_invalidates(pool: PgPool) {
    let (token, _) = init(&pool).await;
    let router = app(pool.clone());
    let response = login(router.clone(), &token).await;
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    for (origin, csrf_header, expected) in [
        ("https://evil.test", "1", StatusCode::FORBIDDEN),
        (ORIGIN, "", StatusCode::FORBIDDEN),
        (ORIGIN, "1", StatusCode::NO_CONTENT),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/admin/v1/clients/absent")
                    .header("cookie", cookie)
                    .header("origin", origin)
                    .header("x-filegate-csrf", csrf_header)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let audit_status: Option<i32> =
        sqlx::query_scalar("SELECT status FROM admin_audit_events WHERE action = 'DELETE'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audit_status, Some(204));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/admin/v1/session")
                .header("cookie", cookie)
                .header("origin", ORIGIN)
                .header("x-filegate-csrf", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        get(router, "/api/admin/v1/clients", "cookie", cookie)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn login_origin_rate_limit_and_disabled_console(pool: PgPool) {
    let (token, _) = init(&pool).await;
    let router = app(pool.clone());
    let request = Request::builder()
        .method("POST")
        .uri("/api/admin/v1/session")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token":token}).to_string()))
        .unwrap();
    assert_eq!(
        router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE admin_login_budget SET attempts = 60")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        login(router.clone(), &token).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    sqlx::query("UPDATE admin_login_budget SET window_start = now() - interval '2 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(login(router, &token).await.status(), StatusCode::OK);
    let mut state = crate::routes::tests::test_state();
    state.pool = pool;
    assert_eq!(
        login(crate::routes::app(state, &[]), &token).await.status(),
        StatusCode::NOT_FOUND
    );
}
