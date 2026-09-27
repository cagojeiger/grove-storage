use super::*;

const PASSWORD: &str = "a private phrase for browser sign-in";

async fn sign_in(router: Router, username: &str, password: &str) -> Response {
    request(
        router,
        "POST",
        PATH,
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"username":username,"password":password}).to_string(),
    )
    .await
}

#[sqlx::test(migrations = "../db/migrations")]
async fn password_login_tracks_role_and_survives_unrelated_token_revocation(pool: PgPool) {
    let (user, token_id, _) = account(&pool, Role::Reader).await;
    grove_management_service::local_accounts::recover(
        &pool,
        Uuid::new_v4(),
        user,
        "reader",
        PASSWORD.into(),
    )
    .await
    .unwrap();
    for (username, password) in [
        ("reader", "wrong phrase with enough characters"),
        ("unknown", PASSWORD),
        ("bad name", PASSWORD),
    ] {
        let response = sign_in(app(&pool), username, password).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    let response = sign_in(app(&pool), " READER ", PASSWORD).await;
    assert_eq!(response.status(), StatusCode::OK);
    let request_id: Uuid = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let cookie = cookie(&response);
    let body = json(response).await;
    assert_eq!(body["credential_id"], serde_json::Value::Null);
    assert_eq!(body["user_id"], user.to_string());
    let detail: (String, Option<Uuid>, Uuid) = sqlx::query_as(
        "SELECT auth_method,credential_id,password_generation FROM management.sessions WHERE id=$1",
    )
    .bind(
        body["session_id"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(detail.0, "password");
    assert_eq!(detail.1, None);
    let audit: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT actor_id,credential_id FROM management.audit_events WHERE request_id=$1",
    )
    .bind(request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, (Some(user), None));
    assert_eq!(current(app(&pool), &cookie).await.status(), StatusCode::OK);
    let accounts_path = "/api/admin/identity/v1/accounts";
    assert_eq!(
        request(
            app(&pool),
            "GET",
            accounts_path,
            &[("cookie", &cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    db::revoke_credential(&pool, &context(), token_id)
        .await
        .unwrap();
    assert_eq!(current(app(&pool), &cookie).await.status(), StatusCode::OK);
    db::change_account(
        &pool,
        &context(),
        user,
        db::AccountChange::Role(Role::Admin),
    )
    .await
    .unwrap();
    assert_eq!(
        request(
            app(&pool),
            "GET",
            accounts_path,
            &[("cookie", &cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        json(current(app(&pool), &cookie).await).await["role"],
        "admin"
    );
    let other = sign_in(app(&pool), "reader", PASSWORD).await;
    let other_cookie = super::cookie(&other);
    assert_eq!(
        logout(app(&pool), &cookie).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        current(app(&pool), &cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        current(app(&pool), &other_cookie).await.status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn recovery_fences_old_password_and_sessions(pool: PgPool) {
    let user = grove_management_service::local_accounts::initialize(
        &pool,
        Uuid::new_v4(),
        "owner",
        "Owner",
        PASSWORD.into(),
    )
    .await
    .unwrap();
    let response = sign_in(app(&pool), "owner", PASSWORD).await;
    assert_eq!(response.status(), StatusCode::OK);
    let old_cookie = cookie(&response);
    let old_generation = db::passwords::find(&pool, "owner")
        .await
        .unwrap()
        .unwrap()
        .generation;
    let replacement = "another long private phrase for recovery";
    grove_management_service::local_accounts::recover(
        &pool,
        Uuid::new_v4(),
        user,
        "owner",
        replacement.into(),
    )
    .await
    .unwrap();
    assert_eq!(
        current(app(&pool), &old_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sign_in(app(&pool), "owner", PASSWORD).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        db::sessions::create_password_session(
            &pool,
            Uuid::new_v4(),
            user,
            old_generation,
            &secrets::session_hash("stale")
        )
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(
        sign_in(app(&pool), "owner", replacement).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        sign_in(app(&pool), "owner", "qwer1234").await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn password_login_rejects_cross_site_and_mixed_credentials(pool: PgPool) {
    grove_management_service::local_accounts::initialize(
        &pool,
        Uuid::new_v4(),
        "owner",
        "Owner",
        PASSWORD.into(),
    )
    .await
    .unwrap();
    for headers in [
        vec![("origin", "https://other.test"), ("x-grove-csrf", "1")],
        vec![("origin", ORIGIN)],
        vec![
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("sec-fetch-site", "cross-site"),
        ],
    ] {
        let response = request(
            app(&pool),
            "POST",
            PATH,
            &headers,
            serde_json::json!({"username":"owner","password":PASSWORD}).to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = request(
        app(&pool),
        "POST",
        PATH,
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"username":"owner","password":PASSWORD,"token":"gsm_abc"}).to_string(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
}

async fn change_password(router: Router, cookie: &str, current: &str, new: &str) -> Response {
    request(
        router,
        "POST",
        "/api/admin/identity/v1/me/password",
        &[
            ("cookie", cookie),
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"current_password":current,"new_password":new}).to_string(),
    )
    .await
}

#[sqlx::test(migrations = "../db/migrations")]
async fn password_change_revokes_browser_sessions_but_keeps_management_tokens(pool: PgPool) {
    let (user, _, _) = account(&pool, Role::Admin).await;
    grove_management_service::local_accounts::recover(
        &pool,
        Uuid::new_v4(),
        user,
        "admin",
        PASSWORD.into(),
    )
    .await
    .unwrap();
    let (token_id, token) = credential(&pool, user).await;
    let first = sign_in(app(&pool), "admin", PASSWORD).await;
    let first_cookie = cookie(&first);
    let second = sign_in(app(&pool), "admin", PASSWORD).await;
    let second_cookie = cookie(&second);
    assert_eq!(
        change_password(
            app(&pool),
            &first_cookie,
            "wrong current password",
            "a new secret phrase for owner"
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        current(app(&pool), &first_cookie).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        change_password(app(&pool), &first_cookie, PASSWORD, PASSWORD)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let new = "a new secret phrase for owner";
    let response = change_password(app(&pool), &first_cookie, PASSWORD, new).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    for cookie in [&first_cookie, &second_cookie] {
        assert_eq!(
            current(app(&pool), cookie).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        sign_in(app(&pool), "admin", PASSWORD).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        sign_in(app(&pool), "admin", new).await.status(),
        StatusCode::OK
    );
    let current_token: bool =
        sqlx::query_scalar("SELECT revoked_at IS NULL FROM management.credentials WHERE id=$1")
            .bind(token_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(current_token);
    assert_eq!(login(app(&pool), &token).await.status(), StatusCode::OK);
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(e)::text FROM management.audit_events e WHERE action='account.password_change'"
    ).fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].contains(PASSWORD));
    assert!(!rows[0].contains(new));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_password_change_audit_preserves_password_and_session(pool: PgPool) {
    grove_management_service::local_accounts::initialize(
        &pool,
        Uuid::new_v4(),
        "owner",
        "Owner",
        PASSWORD.into(),
    )
    .await
    .unwrap();
    let response = sign_in(app(&pool), "owner", PASSWORD).await;
    let cookie = cookie(&response);
    let old_generation = db::passwords::find(&pool, "owner")
        .await
        .unwrap()
        .unwrap()
        .generation;
    sqlx::raw_sql("CREATE FUNCTION management.reject_password_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='account.password_change' THEN RAISE EXCEPTION 'injected audit failure'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER reject_password_audit BEFORE INSERT ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_password_audit();")
        .execute(&pool).await.unwrap();
    let new = "a different phrase for a rejected change";
    assert_eq!(
        change_password(app(&pool), &cookie, PASSWORD, new)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(current(app(&pool), &cookie).await.status(), StatusCode::OK);
    assert_eq!(
        db::passwords::find(&pool, "owner")
            .await
            .unwrap()
            .unwrap()
            .generation,
        old_generation
    );
    assert_eq!(
        sign_in(app(&pool), "owner", PASSWORD).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        sign_in(app(&pool), "owner", new).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
