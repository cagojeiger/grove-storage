use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn reader_and_writer_cannot_manage_identities_or_view_security(pool: PgPool) {
    let (_, admin_cookie) = actor(&pool, Role::Admin).await;
    let target = create(
        &pool,
        &admin_cookie,
        serde_json::json!({"kind":"user","display_name":"target","role":"reader"}),
    )
    .await;
    for role in [Role::Reader, Role::Writer] {
        let (_, cookie) = actor(&pool, role).await;
        for (method, path, body) in [
            (
                "PATCH",
                format!("/accounts/{target}"),
                serde_json::json!({"operation":"name","display_name":"forged"}),
            ),
            ("GET", "/accounts".into(), serde_json::Value::Null),
            (
                "GET",
                format!("/accounts/{target}"),
                serde_json::Value::Null,
            ),
            (
                "POST",
                "/accounts".into(),
                serde_json::json!({"kind":"user","display_name":"forged","role":"admin"}),
            ),
            (
                "PATCH",
                format!("/accounts/{target}"),
                serde_json::json!({"operation":"role","role":"admin"}),
            ),
            (
                "DELETE",
                format!("/accounts/{target}"),
                serde_json::Value::Null,
            ),
            (
                "GET",
                format!("/accounts/{target}/credentials"),
                serde_json::Value::Null,
            ),
            (
                "POST",
                format!("/accounts/{target}/credentials"),
                serde_json::json!({"label":"forged"}),
            ),
            (
                "DELETE",
                format!("/credentials/{}", Uuid::new_v4()),
                serde_json::Value::Null,
            ),
            ("GET", "/history/security".into(), serde_json::Value::Null),
        ] {
            assert_eq!(
                send(&pool, &cookie, method, &path, body).await.status(),
                StatusCode::FORBIDDEN,
                "{method} {path}"
            );
        }
        for path in ["/sessions", "/history/audit", "/history/invocations"] {
            get(&pool, &cookie, path).await;
        }
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn bearer_master_and_cross_site_requests_cannot_bypass_console_boundary(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Admin).await;
    let user_cookie = cookie(&login(app(&pool), &token).await);
    let (router, master_token) = super::super::master::setup(&pool).await;
    let master_cookie = cookie(&super::super::master::sign_in(router.clone(), &master_token).await);
    for path in [
        "/accounts",
        "/accounts/00000000-0000-0000-0000-000000000001",
        "/sessions",
        "/history/audit",
        "/history/invocations",
        "/history/security",
    ] {
        for headers in [
            vec![("authorization", format!("Bearer {token}"))],
            vec![("cookie", master_cookie.clone())],
        ] {
            let borrowed: Vec<_> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
            assert_eq!(
                request(
                    router.clone(),
                    "GET",
                    &format!("{BASE}{path}"),
                    &borrowed,
                    String::new()
                )
                .await
                .status(),
                StatusCode::UNAUTHORIZED
            );
        }
    }
    let response = request(
        router,
        "POST",
        &format!("{BASE}/accounts"),
        &[
            ("cookie", &user_cookie),
            ("origin", "https://evil.test"),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"kind":"user","display_name":"attack","role":"admin"}).to_string(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
