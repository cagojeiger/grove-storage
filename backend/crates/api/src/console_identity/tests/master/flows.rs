use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn bootstrap_delivers_a_user_token_once_and_master_cannot_become_a_user(pool: PgPool) {
    let (router, master_token) = setup(&pool).await;
    let response = sign_in(router.clone(), &master_token).await;
    assert_eq!(response.status(), StatusCode::OK);
    let master_cookie = cookie(&response);
    assert!(master_cookie.starts_with("__Host-grove_setup=gsms_"));
    for flag in ["Secure", "HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(
            response.headers()[header::SET_COOKIE]
                .to_str()
                .unwrap()
                .contains(flag)
        );
    }
    let status = request(
        router.clone(),
        "GET",
        MASTER_PATH,
        &[("cookie", &master_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(json(status).await["initialized"], false);
    assert_eq!(
        current(router.clone(), &master_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let forged = master_cookie.replacen("__Host-grove_setup=gsms_", "__Host-grove_session=gss_", 1);
    assert_eq!(
        current(router.clone(), &forged).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(router.clone(), &master_token).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for path in ["/api/admin/v1/clients", "/api/v1/files"] {
        assert_eq!(
            request(
                router.clone(),
                "GET",
                path,
                &[("cookie", &master_cookie)],
                String::new()
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let response = change(
        router.clone(),
        BOOTSTRAP,
        &master_cookie,
        serde_json::json!({"display_name":"Owner"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let body = json(response).await;
    let user_token = body["token"].as_str().unwrap();
    assert!(secrets::valid(user_token, secrets::TOKEN_PREFIX));
    let user_response = login(router.clone(), user_token).await;
    assert_eq!(user_response.status(), StatusCode::OK);
    let user_cookie = cookie(&user_response);
    assert_eq!(
        json(current(router.clone(), &user_cookie).await).await["role"],
        "admin"
    );
    assert_eq!(
        change(
            router.clone(),
            BOOTSTRAP,
            &master_cookie,
            serde_json::json!({"display_name":"retry"})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            router.clone(),
            "GET",
            MASTER_PATH,
            &[("cookie", &user_cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let master_cookie2 = cookie(&sign_in(router.clone(), &master_token).await);
    assert_eq!(
        change(
            router.clone(),
            BOOTSTRAP,
            &master_cookie2,
            serde_json::json!({"display_name":"second"})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    for table in ["audit_events", "command_invocations", "security_events"] {
        let rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT row_to_json(t)::text FROM management.{table} t"
        ))
        .fetch_all(&pool)
        .await
        .unwrap();
        for row in rows {
            for raw in [
                &master_token,
                user_token,
                master_cookie.split_once('=').unwrap().1,
            ] {
                assert!(!row.contains(raw));
            }
        }
    }
    let ttl: bool = sqlx::query_scalar("SELECT expires_at>created_at+interval '89 days' AND expires_at<=created_at+interval '90 days' FROM management.credentials WHERE id=$1")
        .bind(body["credential_id"].as_str().unwrap().parse::<Uuid>().unwrap()).fetch_one(&pool).await.unwrap();
    assert!(ttl);
}
