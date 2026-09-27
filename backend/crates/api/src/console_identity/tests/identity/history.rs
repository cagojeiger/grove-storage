use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn scoped_history_preserves_bigint_cursors_and_owned_agent_snapshots(pool: PgPool) {
    let (_, admin_cookie) = actor(&pool, Role::Admin).await;
    let (user, user_cookie) = actor(&pool, Role::Reader).await;
    let agent = create(
        &pool,
        &admin_cookie,
        serde_json::json!({"kind":"user","display_name":"historical actor","role":"reader"}),
    )
    .await;
    let key = issue(&pool, &admin_cookie, agent).await;
    let ctx = db::AuditContext {
        actor: db::AuditActor::User {
            id: agent,
            session_id: None,
            credential_id: Some(key["credential_id"].as_str().unwrap().parse().unwrap()),
        },
        request_id: Uuid::new_v4(),
        surface: Surface::Mcp,
    };
    sqlx::query(
        "SELECT setval(pg_get_serial_sequence('management.audit_events','id'),9007199254740993)",
    )
    .execute(&pool)
    .await
    .unwrap();
    db::create_account(
        &pool,
        &ctx,
        db::NewAccount {
            display_name: "historic operation",
            role: Role::Reader,
        },
    )
    .await
    .unwrap();
    db::telemetry::invocation(
        &pool,
        &ctx,
        "storage.list",
        db::telemetry::Outcome::Succeeded,
        None,
        1,
    )
    .await
    .unwrap();
    send(
        &pool,
        &admin_cookie,
        "DELETE",
        &format!("/accounts/{agent}"),
        serde_json::Value::Null,
    )
    .await;
    // Old audit snapshots retain their owner scope after User unification.
    for table in ["audit_events", "command_invocations"] {
        sqlx::query(&format!(
            "UPDATE management.{table} SET actor_kind='agent',owner_user_id=$1 WHERE request_id=$2"
        ))
        .bind(user)
        .bind(ctx.request_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let first = get(&pool, &user_cookie, "/history/audit?limit=1").await;
    for stream in ["audit", "invocations"] {
        for account_id in [agent, user] {
            let filtered = get(
                &pool,
                &user_cookie,
                &format!(
                    "/history/{stream}?account_id={account_id}&credential_id={}&limit=1",
                    key["credential_id"].as_str().unwrap()
                ),
            )
            .await;
            assert_eq!(filtered["items"].as_array().unwrap().len(), 1);
            assert_eq!(
                filtered["items"][0]["context"]["request_id"],
                ctx.request_id.to_string()
            );
        }
    }
    let event = &first["items"][0];
    assert_eq!(event["context"]["request_id"], ctx.request_id.to_string());
    assert_eq!(event["context"]["id"], "9007199254740994");
    assert_eq!(first["next_before"], "9007199254740994");
    let rest = get(
        &pool,
        &user_cookie,
        "/history/audit?limit=100&before=9007199254740994",
    )
    .await;
    assert!(
        rest["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["context"]["actor_id"] == user.to_string()
                || r["context"]["owner_user_id"] == user.to_string())
    );
    let calls = get(&pool, &user_cookie, "/history/invocations?limit=100").await;
    assert!(
        calls["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["context"]["request_id"] == ctx.request_id.to_string())
    );
    assert!(
        calls["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["context"]["actor_id"] == user.to_string()
                || r["context"]["owner_user_id"] == user.to_string())
    );
    let response = send(
        &pool,
        &user_cookie,
        "GET",
        "/history/security",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let denied_id = response.headers()["x-request-id"].to_str().unwrap();
    let security = get(&pool, &admin_cookie, "/history/security?limit=100").await;
    assert!(
        security["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["context"]["request_id"] == denied_id && r["reason_code"] == "forbidden")
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn history_rejects_invalid_filters_and_preserves_security_authorization(pool: PgPool) {
    let (_, cookie) = actor(&pool, Role::Admin).await;
    for stream in ["audit", "invocations", "security"] {
        for query in [
            "account_id=invalid",
            "credential_id=invalid",
            "before=0",
            "before=-1",
            "before=9223372036854775808",
            "limit=0",
            "limit=101",
            "unknown=true",
        ] {
            assert_eq!(
                send(
                    &pool,
                    &cookie,
                    "GET",
                    &format!("/history/{stream}?{query}"),
                    serde_json::Value::Null
                )
                .await
                .status(),
                StatusCode::BAD_REQUEST
            );
        }
        let empty = get(
            &pool,
            &cookie,
            &format!(
                "/history/{stream}?account_id={}&credential_id={}",
                Uuid::new_v4(),
                Uuid::new_v4()
            ),
        )
        .await;
        assert!(empty["items"].as_array().unwrap().is_empty());
    }
    for role in [Role::Reader, Role::Writer] {
        let (id, cookie) = actor(&pool, role).await;
        assert_eq!(
            send(
                &pool,
                &cookie,
                "GET",
                &format!("/history/security?account_id={id}"),
                serde_json::Value::Null
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
}
