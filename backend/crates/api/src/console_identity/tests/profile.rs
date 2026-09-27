use super::*;

const PASSWORD: &str = "a private phrase for profile tests";

#[sqlx::test(migrations = "../db/migrations")]
async fn own_profile_is_password_session_scoped_and_audited(pool: PgPool) {
    let owner = grove_management_service::local_accounts::initialize(
        &pool,
        Uuid::new_v4(),
        "owner",
        "Original",
        PASSWORD.into(),
    )
    .await
    .unwrap();
    let signed_in = request(
        app(&pool),
        "POST",
        PATH,
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"username":"owner","password":PASSWORD}).to_string(),
    )
    .await;
    let session_cookie = cookie(&signed_in);
    let path = "/api/admin/identity/v1/me";
    let profile = request(
        app(&pool),
        "GET",
        path,
        &[("cookie", &session_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(profile.status(), StatusCode::OK);
    let profile = json(profile).await;
    assert_eq!(profile["id"], owner.to_string());
    assert_eq!(profile["username"], "owner");
    assert_eq!(profile["password_ready"], true);
    let sessions = request(
        app(&pool),
        "GET",
        "/api/admin/identity/v1/me/sessions",
        &[("cookie", &session_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(sessions.status(), StatusCode::OK);
    assert_eq!(json(sessions).await["items"].as_array().unwrap().len(), 1);
    let body = serde_json::json!({"display_name":"Updated owner"}).to_string();
    assert_eq!(
        request(
            app(&pool),
            "PATCH",
            path,
            &[
                ("origin", ORIGIN),
                ("x-grove-csrf", "1"),
                ("content-type", "application/json")
            ],
            body.clone()
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            app(&pool),
            "PATCH",
            path,
            &[
                ("origin", "https://elsewhere.test"),
                ("x-grove-csrf", "1"),
                ("cookie", &session_cookie),
                ("content-type", "application/json")
            ],
            body.clone()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let renamed = request(
        app(&pool),
        "PATCH",
        path,
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("cookie", &session_cookie),
            ("content-type", "application/json"),
        ],
        body.clone(),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(json(renamed).await["changed"], true);
    let updated = request(
        app(&pool),
        "GET",
        path,
        &[("cookie", &session_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(json(updated).await["display_name"], "Updated owner");
    let metadata: serde_json::Value = sqlx::query_scalar(
        "SELECT metadata FROM management.audit_events WHERE action='account.name' AND resource_id=$1",
    ).bind(owner.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(metadata["before_name"], "Original");
    assert_eq!(metadata["after_name"], "Updated owner");

    let (_, raw) = credential(&pool, owner).await;
    let legacy = login(app(&pool), &raw).await;
    let legacy_cookie = cookie(&legacy);
    assert_eq!(
        request(
            app(&pool),
            "GET",
            path,
            &[("cookie", &legacy_cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}
