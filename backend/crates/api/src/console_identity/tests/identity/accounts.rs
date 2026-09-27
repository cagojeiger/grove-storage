use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn account_lifecycle_and_last_admin_guard_are_applied_over_http(pool: PgPool) {
    let (admin, cookie) = actor(&pool, Role::Admin).await;
    let user = create(
        &pool,
        &cookie,
        serde_json::json!({"kind":"user","display_name":"Reader","role":"reader"}),
    )
    .await;
    let path = format!("/accounts/{user}");
    assert_eq!(
        send(
            &pool,
            &cookie,
            "PATCH",
            &path,
            serde_json::json!({"operation":"role","role":"admin"})
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            &pool,
            &cookie,
            "PATCH",
            &path,
            serde_json::json!({"operation":"owner","owner_user_id":admin})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    for active in [false, true] {
        assert_eq!(
            send(
                &pool,
                &cookie,
                "PATCH",
                &path,
                serde_json::json!({"operation":"active","is_active":active})
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        send(&pool, &cookie, "DELETE", &path, serde_json::Value::Null)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            &pool,
            &cookie,
            "PATCH",
            &path,
            serde_json::json!({"operation":"active","is_active":true})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let rows = get(&pool, &cookie, "/accounts?limit=100").await;
    assert!(
        rows["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == user.to_string() && !r["deleted_at"].is_null())
    );
    let bootstrap: Uuid =
        sqlx::query_scalar("SELECT id FROM management.accounts WHERE display_name='Owner'")
            .fetch_one(&pool)
            .await
            .unwrap();
    db::change_account(
        &pool,
        &context(),
        bootstrap,
        db::AccountChange::Active(false),
    )
    .await
    .unwrap();
    for (method, body) in [
        ("DELETE", serde_json::Value::Null),
        (
            "PATCH",
            serde_json::json!({"operation":"role","role":"reader"}),
        ),
        (
            "PATCH",
            serde_json::json!({"operation":"active","is_active":false}),
        ),
    ] {
        assert_eq!(
            send(&pool, &cookie, method, &format!("/accounts/{admin}"), body)
                .await
                .status(),
            StatusCode::CONFLICT
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn account_cursor_validation_and_audit_once_contract(pool: PgPool) {
    let (_, cookie) = actor(&pool, Role::Admin).await;
    let target = create(
        &pool,
        &cookie,
        serde_json::json!({"kind":"user","display_name":"target","role":"reader"}),
    )
    .await;
    let response = send(
        &pool,
        &cookie,
        "PATCH",
        &format!("/accounts/{target}"),
        serde_json::json!({"operation":"role","role":"writer"}),
    )
    .await;
    let request_id: Uuid = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for table in ["audit_events", "command_invocations"] {
        let n: i64 = sqlx::query_scalar(&format!(
            "SELECT count(*) FROM management.{table} WHERE request_id=$1"
        ))
        .bind(request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 1);
    }
    let response = send(
        &pool,
        &cookie,
        "PATCH",
        &format!("/accounts/{target}"),
        serde_json::json!({"operation":"role","role":"writer"}),
    )
    .await;
    assert_eq!(json(response).await["changed"], false);
    let first = get(&pool, &cookie, "/accounts?limit=1").await;
    let before = first["next_before"].as_str().unwrap();
    let second = get(
        &pool,
        &cookie,
        &format!("/accounts?limit=1&before={before}"),
    )
    .await;
    assert_ne!(first["items"][0]["id"], second["items"][0]["id"]);
    for path in [
        "/accounts?limit=0",
        "/accounts?limit=101",
        "/accounts?before=secret-invalid",
        "/accounts?limit=1&limit=2",
        "/history/audit?before=-1",
        "/history/audit?actor_id=forged",
        "/me/sessions?owner_user_id=forged",
    ] {
        let response = send(&pool, &cookie, "GET", path, serde_json::Value::Null).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        assert!(!json(response).await.to_string().contains("secret-invalid"));
    }
    for body in [
        serde_json::json!({"kind":"user","display_name":"","role":"admin"}),
        serde_json::json!({"kind":"agent","display_name":"invalid","role":"admin","owner_user_id":target}),
        serde_json::json!({"kind":"agent","display_name":"old writer","role":"writer","owner_user_id":target}),
        serde_json::json!({"kind":"agent","display_name":"old reader","role":"reader","owner_user_id":target}),
        serde_json::json!({"kind":"user","display_name":"invalid owner","role":"reader","owner_user_id":target}),
        serde_json::json!({"kind":"user","display_name":"secret-invalid","role":"admin","actor_id":target}),
    ] {
        let response = send(&pool, &cookie, "POST", "/accounts", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!json(response).await.to_string().contains("secret-invalid"));
    }
}
