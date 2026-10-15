use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn scoped_history_preserves_bigint_cursors_and_account_token_filters(pool: PgPool) {
    let (_, admin_cookie) = actor(&pool, Role::Admin).await;
    let (user, user_cookie) = actor(&pool, Role::Reader).await;
    let agent = create(
        &pool,
        &admin_cookie,
        serde_json::json!({"kind":"user","display_name":"historical actor","role":"reader"}),
    )
    .await;
    let key = issue(&pool, &admin_cookie, user).await;
    let ctx = db::AuditContext {
        actor: db::AuditActor::Account {
            id: user,
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
        &db::telemetry::Details::new("storage.list", db::telemetry::Stage::Execution),
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
            assert_eq!(
                filtered["items"].as_array().unwrap().len(),
                usize::from(account_id == user)
            );
            if account_id == user {
                assert_eq!(
                    filtered["items"][0]["context"]["request_id"],
                    ctx.request_id.to_string()
                );
            }
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
            .all(|r| r["context"]["actor_id"] == user.to_string())
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
            .all(|r| r["context"]["actor_id"] == user.to_string())
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
    let denial = security["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["context"]["request_id"] == denied_id)
        .unwrap();
    assert_eq!(denial["operation"], "history.security.list");
    assert_eq!(denial["stage"], "authorization");
    assert_eq!(denial["auth_method"], "session");
    assert_eq!(denial["is_sampled"], false);
    let missing = Uuid::new_v4();
    let failed = send(
        &pool,
        &admin_cookie,
        "GET",
        &format!("/accounts/{missing}"),
        serde_json::Value::Null,
    )
    .await;
    let failed_id = failed.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(failed.status(), StatusCode::NOT_FOUND);
    let calls = get(&pool, &admin_cookie, "/history/invocations?limit=100").await;
    let failed = calls["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["context"]["request_id"] == failed_id)
        .unwrap();
    assert_eq!(failed["target_type"], "account");
    assert_eq!(failed["target_id"], missing.to_string());
    assert_eq!(failed["stage"], "execution");
    assert_eq!(failed["auth_method"], "session");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn history_rejects_invalid_filters_and_preserves_security_authorization(pool: PgPool) {
    let (_, cookie) = actor(&pool, Role::Admin).await;
    for stream in ["audit", "invocations", "security"] {
        for query in [
            "account_id=invalid",
            "credential_id=invalid",
            "operation_id=invalid",
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

#[sqlx::test(migrations = "../db/migrations")]
async fn operation_history_filter_preserves_scope_actor_filters_and_cursors(pool: PgPool) {
    let (_, admin_cookie) = actor(&pool, Role::Admin).await;
    let (reader, reader_cookie) = actor(&pool, Role::Reader).await;
    let (writer, _) = actor(&pool, Role::Writer).await;
    let own_operation = Uuid::new_v4();
    let foreign_operation = Uuid::new_v4();
    for (id, operation) in [
        (reader, own_operation),
        (writer, foreign_operation),
        (reader, own_operation),
    ] {
        let ctx = db::AuditContext {
            actor: db::AuditActor::Account {
                id,
                session_id: None,
                credential_id: None,
            },
            request_id: operation,
            surface: Surface::Console,
        };
        db::create_account(
            &pool,
            &ctx,
            db::NewAccount {
                display_name: "history target",
                role: Role::Reader,
            },
        )
        .await
        .unwrap();
        let details =
            db::telemetry::Details::new("identity.account.create", db::telemetry::Stage::Execution);
        db::telemetry::invocation(
            &pool,
            &ctx,
            &details,
            db::telemetry::Outcome::Succeeded,
            None,
            1,
        )
        .await
        .unwrap();
        db::telemetry::security(
            &pool,
            Some(&ctx),
            operation,
            Surface::Console,
            db::telemetry::SecurityReason::Authenticated,
            &details,
        )
        .await
        .unwrap();
    }
    for stream in ["audit", "invocations", "security"] {
        let path = format!("/history/{stream}?operation_id={own_operation}&limit=1");
        let first = get(&pool, &admin_cookie, &path).await;
        assert_eq!(first["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            first["items"][0]["context"]["request_id"],
            own_operation.to_string()
        );
        let before = first["next_before"].as_str().unwrap();
        let second = get(&pool, &admin_cookie, &format!("{path}&before={before}")).await;
        assert_eq!(second["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            second["items"][0]["context"]["request_id"],
            own_operation.to_string()
        );
        assert_ne!(
            first["items"][0]["context"]["id"],
            second["items"][0]["context"]["id"]
        );
        let before = second["next_before"].as_str().unwrap();
        assert!(
            get(&pool, &admin_cookie, &format!("{path}&before={before}")).await["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        for other_filter in [
            format!("account_id={writer}"),
            format!("credential_id={}", Uuid::new_v4()),
        ] {
            assert!(
                get(&pool, &admin_cookie, &format!("{path}&{other_filter}")).await["items"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        if stream == "security" {
            assert_eq!(
                send(&pool, &reader_cookie, "GET", &path, serde_json::Value::Null)
                    .await
                    .status(),
                StatusCode::FORBIDDEN
            );
        } else {
            let own = get(&pool, &reader_cookie, &path).await;
            assert_eq!(own["items"][0]["context"]["actor_id"], reader.to_string());
            let foreign = get(
                &pool,
                &reader_cookie,
                &format!("/history/{stream}?operation_id={foreign_operation}"),
            )
            .await;
            assert!(foreign["items"].as_array().unwrap().is_empty());
        }
    }
}
