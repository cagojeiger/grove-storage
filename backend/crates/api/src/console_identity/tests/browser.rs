use super::*;

#[test]
fn token_syntax_and_hash_domains_are_distinct() {
    let raw = format!("gsm_{}", "a".repeat(64));
    assert!(secrets::valid(&raw, secrets::TOKEN_PREFIX));
    for invalid in [
        raw.to_uppercase(),
        format!("{raw} "),
        raw.replacen("gsm_", "gss_", 1),
        "gsm_short".into(),
    ] {
        assert!(!secrets::valid(&invalid, secrets::TOKEN_PREFIX));
    }
    assert_ne!(secrets::token_hash(&raw), secrets::session_hash(&raw));
    assert_ne!(
        secrets::token_hash(&raw),
        crate::admin_auth::hash("admin-token", &raw)
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn browser_admission_rejects_ambiguous_and_cross_site_requests(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Reader).await;
    let cookie = cookie(&login(app(&pool), &token).await);
    for extra in [
        vec![],
        vec![("origin", ORIGIN)],
        vec![("origin", "https://evil.test"), ("x-grove-csrf", "1")],
        vec![
            ("origin", ORIGIN),
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
        ],
        vec![
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("x-grove-csrf", "1"),
        ],
        vec![
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("sec-fetch-site", "same-site"),
        ],
    ] {
        for method in ["POST", "DELETE"] {
            let mut headers = extra.clone();
            headers.extend([
                ("content-type", "application/json"),
                ("cookie", cookie.as_str()),
            ]);
            let response = request(
                app(&pool),
                method,
                PATH,
                &headers,
                serde_json::json!({"token":token}).to_string(),
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{method}: {extra:?}"
            );
            assert!(!response.headers().contains_key(header::SET_COOKIE));
        }
    }
    for (key, value) in [
        ("origin", "https://evil.test"),
        ("sec-fetch-site", "cross-site"),
    ] {
        assert_eq!(
            request(
                app(&pool),
                "GET",
                PATH,
                &[("cookie", &cookie), (key, value)],
                String::new()
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(current(app(&pool), &cookie).await.status(), StatusCode::OK);
    let supplied_id = Uuid::new_v4().to_string();
    let response = request(
        app(&pool),
        "GET",
        PATH,
        &[("cookie", &cookie), ("x-request-id", &supplied_id)],
        String::new(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_ne!(response.headers()["x-request-id"], supplied_id);
    for headers in [
        vec![("cookie", cookie.as_str()), ("cookie", cookie.as_str())],
        vec![
            ("cookie", cookie.as_str()),
            ("authorization", "Bearer anything"),
        ],
        vec![
            ("x-auth-request-user", "admin"),
            ("x-forwarded-user", "admin"),
        ],
    ] {
        assert_eq!(
            request(app(&pool), "GET", PATH, &headers, String::new())
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let duplicate = format!("{cookie}; {cookie}");
    assert_eq!(
        current(app(&pool), &duplicate).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let response = request(
        app(&pool),
        "OPTIONS",
        PATH,
        &[
            ("origin", "https://evil.test"),
            ("access-control-request-method", "POST"),
        ],
        String::new(),
    )
    .await;
    assert!(
        !response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    assert!(!response.status().is_success());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn legacy_and_data_authority_remain_separate(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Admin).await;
    let new_cookie = cookie(&login(app(&pool), &token).await);
    let old_raw = format!("fgop_{}", filegate_core::generate_url_secret());
    filegate_db::admin_auth::issue(
        &pool,
        filegate_db::admin_auth::IssueMode::Initialize,
        "old",
        &crate::admin_auth::hash("admin-token", &old_raw),
    )
    .await
    .unwrap();
    let old_login = request(
        app(&pool),
        "POST",
        "/api/admin/v1/session",
        &[
            ("origin", ORIGIN),
            ("x-filegate-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"token":old_raw}).to_string(),
    )
    .await;
    assert_eq!(old_login.status(), StatusCode::OK);
    let old_cookie = cookie(&old_login);
    assert_eq!(
        current(app(&pool), &old_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(app(&pool), &old_raw).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for path in ["/api/admin/v1/clients", "/api/v1/files"] {
        assert_eq!(
            request(
                app(&pool),
                "GET",
                path,
                &[("cookie", &new_cookie)],
                String::new()
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            request(
                app(&pool),
                "GET",
                path,
                &[("authorization", &format!("Bearer {token}"))],
                String::new()
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(
            app(&pool),
            "GET",
            "/api/admin/v1/clients",
            &[("cookie", &old_cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::OK
    );
    let swapped = old_cookie.replacen("__Host-filegate_session", COOKIE, 1);
    assert_eq!(
        current(app(&pool), &swapped).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let forged = format!("{COOKIE}={token}");
    assert_eq!(
        current(app(&pool), &forged).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
