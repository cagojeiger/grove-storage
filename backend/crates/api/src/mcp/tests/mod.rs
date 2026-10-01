#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod boundaries;
mod catalog;
mod failures;
mod protocol;
mod resources;

use super::*;
use crate::resource_commands::tests::{owner, seed};
use axum::{body::to_bytes, http::Request as HttpRequest};
use filegate_db::{PgPool, management as db};
use grove_management_policy::{Role, Surface};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const PATH: &str = "/api/admin/mcp";
const VERSION: &str = "2026-07-28";

async fn rpc(pool: &PgPool, token: &str, method: &str, mut params: Value) -> Response {
    params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":VERSION,"io.modelcontextprotocol/clientCapabilities":{}});
    let mut headers = vec![
        ("authorization", format!("Bearer {token}")),
        ("mcp-method", method.to_owned()),
        ("mcp-protocol-version", VERSION.to_owned()),
    ];
    if let Some(name) = params.get("name").and_then(Value::as_str) {
        headers.push(("mcp-name", name.to_owned()));
    }
    request(
        pool,
        "POST",
        &headers,
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
    )
    .await
}

async fn call(pool: &PgPool, token: &str, name: &str, input: Value) -> Value {
    let response = rpc(
        pool,
        token,
        "tools/call",
        json!({"name":name,"arguments":input}),
    )
    .await;
    let status = response.status();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!response.headers().contains_key("mcp-session-id"));
    let body = json_body(response).await;
    assert_eq!(status, StatusCode::OK, "{name}: {body}");
    assert!(body.get("error").is_none(), "{body}");
    body["result"].clone()
}

async fn request(pool: &PgPool, method: &str, headers: &[(&str, String)], body: Value) -> Response {
    let mut state = crate::routes::tests::test_state();
    state.pool = pool.clone();
    let mut request = HttpRequest::builder()
        .method(method)
        .uri(PATH)
        .header("accept", "application/json, text/event-stream")
        .header("content-type", "application/json");
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        request = request.header("host", "127.0.0.1");
    }
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    crate::routes::app(state, &[])
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn json_body(response: Response) -> Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap()
}

fn context() -> db::AuditContext {
    db::AuditContext {
        actor: db::AuditActor::Master { session_id: None },
        request_id: Uuid::new_v4(),
        surface: Surface::Console,
    }
}
