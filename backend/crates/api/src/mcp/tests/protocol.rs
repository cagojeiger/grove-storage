use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn discovery_and_legacy_initialize_remain_stateless(pool: PgPool) {
    let token = owner(&pool).await;
    let response = rpc(&pool, &token, "server/discover", json!({})).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("mcp-session-id"));
    let body = json_body(response).await;
    assert!(
        body["result"]["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!(VERSION))
    );
    assert!(body["result"]["capabilities"].get("tools").is_some());
    let headers = [
        ("authorization", format!("Bearer {token}")),
        ("mcp-protocol-version", "2025-11-25".into()),
    ];
    let response = request(&pool,"POST",&headers,json!({
        "jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"legacy-test","version":"1"}
        }
    })).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("mcp-session-id"));
    let body = json_body(response).await;
    assert_eq!(body["result"]["protocolVersion"], "2025-11-25");
    let response = request(
        &pool,
        "POST",
        &headers,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        24
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn sdk_rejects_host_and_header_body_mismatch_before_execution(pool: PgPool) {
    let token = owner(&pool).await;
    let headers = vec![
        ("authorization", format!("Bearer {token}")),
        ("mcp-protocol-version", VERSION.into()),
        ("mcp-method", "tools/call".into()),
        ("mcp-name", "client.delete".into()),
    ];
    let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "name":"client.create","arguments":{"id":"app","storage_id":"local"},
        "_meta":{"io.modelcontextprotocol/protocolVersion":VERSION,"io.modelcontextprotocol/clientCapabilities":{}}
    }});
    assert_eq!(
        request(&pool, "POST", &headers, body.clone())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let mut headers = headers;
    headers.push(("host", "untrusted.example".into()));
    assert_eq!(
        request(&pool, "POST", &headers, body).await.status(),
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
