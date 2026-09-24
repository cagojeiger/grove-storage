use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn stale_replica_returns_unavailable_and_rotated_sessions_are_rejected(pool: PgPool) {
    let (old_router, old_token) = setup(&pool).await;
    let old_cookie = cookie(&sign_in(old_router.clone(), &old_token).await);
    let token = format!("gsmt_{}", filegate_core::generate_url_secret());
    let config = Arc::new(Config::new(2, secrets::master_hash(&token)).unwrap());
    config.install(&pool).await.unwrap();
    let new_router = router(&pool, config);
    assert_eq!(
        sign_in(old_router, &old_token).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        request(
            new_router.clone(),
            "GET",
            MASTER_PATH,
            &[("cookie", &old_cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(sign_in(new_router, &token).await.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn master_routes_enforce_browser_checks_disabled_config_and_logout(pool: PgPool) {
    let (router, token) = setup(&pool).await;
    assert_eq!(
        sign_in(app(&pool), &token).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        sign_in(router.clone(), "wrong").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let master_cookie = cookie(&sign_in(router.clone(), &token).await);
    for path in [MASTER_PATH, BOOTSTRAP, RECOVER] {
        let response = request(
            router.clone(),
            "POST",
            path,
            &[
                ("origin", "https://evil.test"),
                ("x-grove-csrf", "1"),
                ("cookie", &master_cookie),
                ("content-type", "application/json"),
            ],
            "{}".into(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = request(
            router.clone(),
            "POST",
            path,
            &[
                ("origin", ORIGIN),
                ("cookie", &master_cookie),
                ("content-type", "application/json"),
            ],
            "{}".into(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    for headers in [
        vec![
            ("cookie", master_cookie.as_str()),
            ("authorization", "Bearer irrelevant"),
        ],
        vec![
            ("cookie", master_cookie.as_str()),
            ("cookie", master_cookie.as_str()),
        ],
    ] {
        assert_eq!(
            request(router.clone(), "GET", MASTER_PATH, &headers, String::new())
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let response = request(
        router.clone(),
        "DELETE",
        MASTER_PATH,
        &[
            ("cookie", &master_cookie),
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
        ],
        String::new(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        request(
            router,
            "GET",
            MASTER_PATH,
            &[("cookie", &master_cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
