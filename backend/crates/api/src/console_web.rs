use std::{path::PathBuf, sync::Arc};

use axum::{
    Router,
    extract::{OriginalUri, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use tower_http::services::{ServeDir, ServeFile};

use crate::routes::AppState;

const NONCE: &str = "__GROVE_CSP_NONCE__";
const BASE_CSP: &str = "default-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self'; style-src-attr 'none'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

pub(crate) fn parse_origin(value: Option<String>) -> anyhow::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let uri: axum::http::Uri = value.parse()?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https")
            && uri.host().is_some()
            && uri.path() == "/"
            && uri.query().is_none()
            && !value.contains(['@', '#', '?'])
            && !value.ends_with('/'),
        "GROVE_CONSOLE_ORIGIN must be an HTTPS origin without a trailing slash"
    );
    Ok(Some(value))
}

#[derive(Clone)]
pub(crate) struct ConsoleWeb {
    directory: PathBuf,
    index: Arc<str>,
}

impl ConsoleWeb {
    pub(crate) async fn load(directory: PathBuf) -> anyhow::Result<Self> {
        let directory = tokio::fs::canonicalize(directory).await?;
        let index = tokio::fs::read_to_string(directory.join("index.html")).await?;
        anyhow::ensure!(
            index.matches(NONCE).count() == 1,
            "console index must contain one CSP nonce placeholder"
        );
        anyhow::ensure!(
            tokio::fs::metadata(directory.join("assets"))
                .await?
                .is_dir(),
            "console assets directory missing"
        );
        Ok(Self {
            directory,
            index: index.into(),
        })
    }
}

pub(crate) fn validate_object_host(origin: &str, public_url: Option<&str>) -> anyhow::Result<()> {
    let Some(public_url) = public_url else {
        return Ok(());
    };
    let console: axum::http::Uri = origin.parse()?;
    let object: axum::http::Uri = public_url.parse()?;
    anyhow::ensure!(
        matches!(object.scheme_str(), Some("http" | "https")) && object.host().is_some(),
        "GROVE_PUBLIC_URL must be an http(s) URL"
    );
    // URL clients omit default ports from Host; the boundary treats absent ports as 443.
    let object_port = object
        .port_u16()
        .filter(|port| {
            !matches!(
                (object.scheme_str(), *port),
                (Some("http"), 80) | (Some("https"), 443)
            )
        })
        .unwrap_or(443);
    let same_host = console
        .host()
        .zip(object.host())
        .is_some_and(|(console, object)| console.eq_ignore_ascii_case(object));
    anyhow::ensure!(
        !same_host || console.port_u16().unwrap_or(443) != object_port,
        "GROVE_PUBLIC_URL must not use the console host"
    );
    Ok(())
}

pub(crate) fn routes(state: &AppState) -> Router<AppState> {
    let Some(web) = &state.console_web else {
        return Router::new();
    };
    let mut router = Router::new()
        .route("/api/admin/console", get(index))
        .route("/api/admin/console/", get(index))
        .route("/api/admin/console/index.html", get(index))
        .nest_service(
            "/api/admin/console/assets",
            ServeDir::new(web.directory.join("assets")),
        );
    for file in [
        "grove-storage-logo.png",
        "MUI-TEMPLATE-LICENSE.txt",
        "SWAGGER-UI-LICENSE.txt",
        "SWAGGER-UI-NOTICE.txt",
    ] {
        router = router.route_service(
            &format!("/api/admin/console/{file}"),
            ServeFile::new(web.directory.join(file)),
        );
    }
    router.layer(middleware::from_fn(security_headers))
}

async fn index(State(state): State<AppState>, OriginalUri(uri): OriginalUri) -> Response {
    if uri.path() == "/api/admin/console" {
        return Redirect::permanent("/api/admin/console/").into_response();
    }
    let Some(web) = &state.console_web else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let nonce = grove_core::generate_url_secret();
    let mut response = Html(web.index.replace(NONCE, &nonce)).into_response();
    let csp = format!("{BASE_CSP}; style-src-elem 'self' 'nonce-{nonce}'");
    if let Ok(value) = HeaderValue::from_str(&csp) {
        response
            .headers_mut()
            .insert("content-security-policy", value);
    }
    response
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers
        .entry("content-security-policy")
        .or_insert(HeaderValue::from_static(BASE_CSP));
    for (name, value) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    response
}

// The ingress preserves Host; forwarded host/protocol headers are not authority.
pub(crate) async fn host_boundary(
    State(origin): State<String>,
    request: Request,
    next: Next,
) -> Response {
    let configured = origin.parse::<axum::http::Uri>().ok();
    let expected = configured.as_ref().and_then(|uri| uri.authority());
    let mut hosts = request.headers().get_all(header::HOST).iter();
    let host = hosts
        .next()
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.contains('@'))
        .and_then(|value| value.parse::<axum::http::uri::Authority>().ok());
    if host.is_none() || hosts.next().is_some() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let console_host = host
        .as_ref()
        .zip(expected)
        .is_some_and(|(actual, expected)| {
            actual.host().eq_ignore_ascii_case(expected.host())
                && actual.port_u16().unwrap_or(443) == expected.port_u16().unwrap_or(443)
        });
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map(|uri| uri.0.path())
        .unwrap_or_else(|| request.uri().path());
    if console_host && path == "/" && matches!(request.method(), &Method::GET | &Method::HEAD) {
        return (
            [(header::CACHE_CONTROL, "no-store")],
            Redirect::temporary("/api/admin/console/"),
        )
            .into_response();
    }
    let console_path = [
        "/api/admin/console",
        "/api/admin/identity/v1",
        "/api/admin/console-commands/v1",
    ]
    .iter()
    .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")));
    let shared_path =
        matches!(path, "/" | "/healthz" | "/readyz") || path.starts_with("/api/docs/");
    if (!console_host && console_path) || (console_host && !console_path && !shared_path) {
        return (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, "no-store")]).into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests;
