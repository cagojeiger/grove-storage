#![allow(clippy::unwrap_used)]

use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

use super::*;

#[test]
fn console_origin_requires_an_https_origin() {
    assert!(parse_origin(None).unwrap().is_none());
    assert!(parse_origin(Some("https://console.test".into())).is_ok());
    for origin in [
        "http://console.test",
        "https://console.test/",
        "https://console.test/path",
        "https://user@console.test",
        "https://console.test?x",
        "https://console.test#x",
        "null",
    ] {
        assert!(parse_origin(Some(origin.into())).is_err(), "{origin}");
    }
}

#[test]
fn public_object_url_cannot_select_the_console_host_boundary() {
    let origin = "https://console.test";
    for public in [
        "https://console.test",
        "https://CONSOLE.TEST:443/objects",
        "http://console.test",
        "http://console.test:80/objects",
    ] {
        assert!(
            validate_object_host(origin, Some(public)).is_err(),
            "{public}"
        );
    }
    for public in [
        "https://objects.test",
        "http://objects.test:8080",
        "https://console.test:8443",
    ] {
        assert!(
            validate_object_host(origin, Some(public)).is_ok(),
            "{public}"
        );
    }
    assert!(validate_object_host(origin, None).is_ok());
    assert!(validate_object_host(origin, Some("not-a-url")).is_err());
    assert!(
        validate_object_host(
            "https://console.test:8443",
            Some("https://console.test:8443")
        )
        .is_err()
    );
    assert!(
        validate_object_host("https://console.test:8443", Some("https://console.test")).is_ok()
    );
    assert!(validate_object_host("https://[::1]", Some("https://[::1]:443/objects")).is_err());
    assert!(validate_object_host("https://[::1]:8443", Some("http://[::1]:8080")).is_ok());
}

struct Fixture(PathBuf);
impl Fixture {
    async fn new() -> Self {
        let path = std::env::temp_dir().join(format!("grove-console-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(path.join("assets"))
            .await
            .unwrap();
        tokio::fs::write(
            path.join("index.html"),
            format!("<meta name=\"csp-nonce\" content=\"{NONCE}\"><div id=\"root\"></div>"),
        )
        .await
        .unwrap();
        tokio::fs::write(path.join("assets/app.js"), "export const ready = true;")
            .await
            .unwrap();
        Self(path)
    }
    async fn app(&self) -> Router {
        let mut state = crate::routes::tests::test_state();
        state.console_origin = Some("https://console.test".into());
        state.console_web = Some(ConsoleWeb::load(self.0.clone()).await.unwrap());
        crate::routes::app(state, &[])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

async fn get(app: &Router, path: &str, host: &str) -> Response {
    app.clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("host", host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn text(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn console_root_redirects_get_and_head_to_the_console() {
    let fixture = Fixture::new().await;
    let app = fixture.app().await;
    for host in ["console.test", "CONSOLE.TEST:443"] {
        for method in ["GET", "HEAD"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/")
                        .header("host", host)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
            assert_eq!(response.headers()["location"], "/api/admin/console/");
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert!(text(response).await.is_empty());
        }
    }
    let post = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("host", "console.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn object_root_keeps_service_information_despite_forwarded_host() {
    let fixture = Fixture::new().await;
    let app = fixture.app().await;
    for host in ["objects.test", "console.test:444", "console.test.evil"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("host", host)
                    .header("x-forwarded-host", "console.test")
                    .header("forwarded", "host=console.test;proto=https")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key("location"));
        let body: serde_json::Value = serde_json::from_str(&text(response).await).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"name": "grove-storage", "version": env!("CARGO_PKG_VERSION")})
        );
    }
}

#[tokio::test]
async fn index_has_fresh_nonce_restrictive_headers_and_head_support() {
    let fixture = Fixture::new().await;
    let app = fixture.app().await;
    let first = get(&app, "/api/admin/console/", "console.test").await;
    assert_eq!(first.status(), StatusCode::OK);
    let csp = first.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(!csp.contains("unsafe-inline"));
    assert!(csp.contains("script-src 'self'"));
    assert!(csp.contains("style-src-attr 'none'"));
    assert_eq!(first.headers()["cache-control"], "no-store");
    assert_eq!(first.headers()["x-content-type-options"], "nosniff");
    assert_eq!(first.headers()["x-frame-options"], "DENY");
    assert_eq!(first.headers()["referrer-policy"], "no-referrer");
    assert!(first.headers().contains_key("x-request-id"));
    let html = text(first).await;
    assert!(!html.contains(NONCE));
    let nonce = html
        .split("content=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(csp.contains(&format!("'nonce-{nonce}'")));
    let second = get(&app, "/api/admin/console/index.html", "CONSOLE.TEST:443").await;
    assert_eq!(second.status(), StatusCode::OK);
    assert_ne!(second.headers()["content-security-policy"], csp);
    let head = app
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/api/admin/console/")
                .header("host", "console.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert!(text(head).await.is_empty());
}

#[tokio::test]
async fn assets_are_served_without_spa_or_path_traversal_fallbacks() {
    let fixture = Fixture::new().await;
    let app = fixture.app().await;
    let response = get(&app, "/api/admin/console/assets/app.js", "console.test").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/javascript");
    assert_eq!(text(response).await, "export const ready = true;");
    for path in [
        "/api/admin/console/assets/missing.js",
        "/api/admin/console/../index.html",
        "/api/admin/console/%2e%2e/index.html",
        "/api/admin/console/unknown",
    ] {
        assert_eq!(
            get(&app, path, "console.test").await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    let redirect = get(&app, "/api/admin/console", "console.test").await;
    assert_eq!(redirect.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(redirect.headers()["location"], "/api/admin/console/");
}

#[tokio::test]
async fn management_host_cannot_serve_objects_and_object_host_cannot_serve_console() {
    let fixture = Fixture::new().await;
    let app = fixture.app().await;
    for path in [
        "/api/admin/console/",
        "/api/admin/identity/v1/session",
        "/api/admin/console-commands/v1",
    ] {
        assert_eq!(
            get(&app, path, "objects.test").await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    for path in ["/test-client/object.html", "/api/v1/files", "/blobs/lease"] {
        assert_eq!(
            get(&app, path, "console.test").await.status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    let object = get(&app, "/test-client/object.html", "objects.test").await;
    assert_eq!(object.status(), StatusCode::FORBIDDEN);
    assert_eq!(object.headers()["content-type"], "application/xml");
    let spoof = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/console/")
                .header("host", "objects.test")
                .header("x-forwarded-host", "console.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spoof.status(), StatusCode::NOT_FOUND);
    for host in ["console.test:444", "console.test.evil", "user@console.test"] {
        assert!(
            !get(&app, "/api/admin/console/", host)
                .await
                .status()
                .is_success()
        );
    }
    assert_eq!(
        get(&app, "/healthz", "console.test").await.status(),
        StatusCode::OK
    );
    let missing = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/console/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
    let duplicate = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/console/")
                .header("host", "console.test")
                .header("host", "objects.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn disabled_console_and_incomplete_build_fail_closed() {
    let mut state = crate::routes::tests::test_state();
    state.console_origin = Some("https://console.test".into());
    let app = crate::routes::app(state, &[]);
    assert_eq!(
        get(&app, "/", "console.test").await.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    for path in ["/test-client/object.html", "/api/v1/files", "/blobs/lease"] {
        assert_eq!(
            get(&app, path, "console.test").await.status(),
            StatusCode::NOT_FOUND
        );
    }
    for path in [
        "/api/admin/identity/v1/session",
        "/api/admin/console-commands/v1",
    ] {
        assert_eq!(
            get(&app, path, "objects.test").await.status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        get(&app, "/api/admin/console/", "console.test")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let fixture = Fixture::new().await;
    for html in ["<div>missing</div>".to_owned(), NONCE.repeat(2)] {
        tokio::fs::write(fixture.0.join("index.html"), html)
            .await
            .unwrap();
        assert!(ConsoleWeb::load(fixture.0.clone()).await.is_err());
    }
}
