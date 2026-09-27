use super::*;

const OWNER_PASSWORD: &str = "a private phrase for account setup";
const USER_PASSWORD: &str = "a different private phrase for new user";

async fn admin(pool: &PgPool) -> (Uuid, String) {
    let account = grove_management_service::local_accounts::initialize(
        pool,
        Uuid::new_v4(),
        "owner",
        "Owner",
        OWNER_PASSWORD.into(),
    )
    .await
    .unwrap();
    let response = post(
        app(pool),
        PATH,
        None,
        serde_json::json!({"username":"owner","password":OWNER_PASSWORD}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    (account, cookie(&response))
}

async fn post(
    router: Router,
    path: &str,
    cookie: Option<&str>,
    body: serde_json::Value,
) -> Response {
    let mut headers = vec![
        ("origin", ORIGIN),
        ("x-grove-csrf", "1"),
        ("content-type", "application/json"),
    ];
    if let Some(cookie) = cookie {
        headers.push(("cookie", cookie));
    }
    request(router, "POST", path, &headers, body.to_string()).await
}

fn issue_path(id: Uuid) -> String {
    format!("/api/admin/identity/v1/accounts/{id}/password-setup")
}

async fn enable_admin_password(pool: &PgPool, id: Uuid) {
    let username = format!("admin{}", id.simple());
    let hash = grove_management_service::passwords::hash(&username, OWNER_PASSWORD.into())
        .await
        .unwrap();
    db::passwords::recover(
        pool,
        Uuid::new_v4(),
        id,
        &username,
        filegate_core::ExposeSecret::expose_secret(&hash),
    )
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../db/migrations")]
async fn setup_link_is_one_time_and_creates_a_login_without_browser_session(pool: PgPool) {
    let (_, admin_cookie) = admin(&pool).await;
    let target = db::create_account(
        &pool,
        &context(),
        db::NewAccount {
            display_name: "New user",
            role: Role::Writer,
        },
    )
    .await
    .unwrap();
    let path = issue_path(target);
    let wrong = post(
        app(&pool),
        &path,
        Some(&admin_cookie),
        serde_json::json!({"username":"writer","current_password":"wrong private phrase"}),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let issued = post(
        app(&pool),
        &path,
        Some(&admin_cookie),
        serde_json::json!({"username":"writer","current_password":OWNER_PASSWORD}),
    )
    .await;
    assert_eq!(issued.status(), StatusCode::OK);
    let value = json(issued).await;
    let token = value["token"].as_str().unwrap();
    assert!(secrets::valid(token, secrets::SETUP_PREFIX));
    assert_eq!(value["username"], "writer");
    assert_eq!(value["account_id"], target.to_string());
    let stored: String = sqlx::query_scalar(
        "SELECT token_hash FROM management.password_setup_tokens WHERE account_id=$1",
    )
    .bind(target)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, secrets::setup_hash(token));
    assert!(!stored.contains(token));
    assert_eq!(
        post(
            app(&pool),
            PATH,
            None,
            serde_json::json!({"username":"writer","password":USER_PASSWORD})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED,
    );
    let inspect_path = "/api/admin/identity/v1/password-setup/inspect";
    let complete_path = "/api/admin/identity/v1/password-setup";
    let info = post(
        app(&pool),
        inspect_path,
        None,
        serde_json::json!({"token":token}),
    )
    .await;
    assert_eq!(info.status(), StatusCode::OK);
    assert_eq!(json(info).await["username"], "writer");
    let weak = post(
        app(&pool),
        complete_path,
        None,
        serde_json::json!({"token":token,"password":"qwer1234"}),
    )
    .await;
    assert_eq!(weak.status(), StatusCode::BAD_REQUEST);
    let completed = post(
        app(&pool),
        complete_path,
        None,
        serde_json::json!({"token":token,"password":USER_PASSWORD}),
    )
    .await;
    assert_eq!(completed.status(), StatusCode::NO_CONTENT);
    assert!(!completed.headers().contains_key(header::SET_COOKIE));
    assert_eq!(
        post(
            app(&pool),
            inspect_path,
            None,
            serde_json::json!({"token":token})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(
            app(&pool),
            complete_path,
            None,
            serde_json::json!({"token":token,"password":USER_PASSWORD})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let login = post(
        app(&pool),
        PATH,
        None,
        serde_json::json!({"username":"writer","password":USER_PASSWORD}),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    assert_eq!(json(login).await["user_id"], target.to_string());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn setup_issuance_requires_admin_password_session_and_same_origin(pool: PgPool) {
    let (owner, admin_cookie) = admin(&pool).await;
    let target = db::create_account(
        &pool,
        &context(),
        db::NewAccount {
            display_name: "Target",
            role: Role::Reader,
        },
    )
    .await
    .unwrap();
    let path = issue_path(target);
    let input = serde_json::json!({"username":"reader","current_password":OWNER_PASSWORD});
    let cross_site = request(
        app(&pool),
        "POST",
        &path,
        &[
            ("origin", "https://other.test"),
            ("x-grove-csrf", "1"),
            ("cookie", &admin_cookie),
        ],
        input.to_string(),
    )
    .await;
    assert_eq!(cross_site.status(), StatusCode::FORBIDDEN);
    let no_csrf = request(
        app(&pool),
        "POST",
        &path,
        &[("origin", ORIGIN), ("cookie", &admin_cookie)],
        input.to_string(),
    )
    .await;
    assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        post(app(&pool), &path, None, input.clone()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let (credential_id, token) = credential(&pool, owner).await;
    assert_eq!(
        login(app(&pool), &token).await.status(),
        StatusCode::BAD_REQUEST
    );
    db::revoke_credential(&pool, &context(), credential_id)
        .await
        .unwrap();
    let second_admin = db::create_account(
        &pool,
        &context(),
        db::NewAccount {
            display_name: "Other admin",
            role: Role::Admin,
        },
    )
    .await
    .unwrap();
    assert_ne!(second_admin, owner);
    enable_admin_password(&pool, second_admin).await;
    db::change_account(
        &pool,
        &context(),
        owner,
        db::AccountChange::Role(Role::Writer),
    )
    .await
    .unwrap();
    assert_eq!(
        post(app(&pool), &path, Some(&admin_cookie), input)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn account_creation_issues_setup_link_atomically(pool: PgPool) {
    let (owner, admin_cookie) = admin(&pool).await;
    let path = "/api/admin/identity/v1/accounts";
    let create = |username: &str, password: &str| {
        serde_json::json!({
            "kind": "user_with_password_setup",
            "display_name": "New reader",
            "role": "reader",
            "username": username,
            "current_password": password,
        })
    };
    assert_eq!(
        post(app(&pool), path, None, create("reader", OWNER_PASSWORD))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            app(&pool),
            path,
            Some(&admin_cookie),
            create("reader", "wrong")
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let created = post(
        app(&pool),
        path,
        Some(&admin_cookie),
        create("reader", OWNER_PASSWORD),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let request_id: Uuid = created.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let value = json(created).await;
    let account: Uuid = value["account_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(value["username"], "reader");
    let token = value["token"].as_str().unwrap();
    assert!(secrets::valid(token, secrets::SETUP_PREFIX));
    let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations WHERE request_id=$1 AND operation='identity.account.create' AND outcome='succeeded'")
        .bind(request_id).fetch_one(&pool).await.unwrap();
    assert_eq!(invocations, 1);
    let details = request(
        app(&pool),
        "GET",
        &format!("/api/admin/identity/v1/accounts/{account}"),
        &[("cookie", &admin_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(details.status(), StatusCode::OK);
    let details = json(details).await;
    assert_eq!(details["username"], "reader");
    assert_eq!(details["password_ready"], false);
    assert_eq!(
        post(
            app(&pool),
            path,
            Some(&admin_cookie),
            create("reader", OWNER_PASSWORD)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM management.accounts WHERE display_name='New reader'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    let completed = post(
        app(&pool),
        "/api/admin/identity/v1/password-setup",
        None,
        serde_json::json!({"token":token,"password":USER_PASSWORD}),
    )
    .await;
    assert_eq!(completed.status(), StatusCode::NO_CONTENT);
    let details = request(
        app(&pool),
        "GET",
        &format!("/api/admin/identity/v1/accounts/{account}"),
        &[("cookie", &admin_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(json(details).await["password_ready"], true);
    let login = post(
        app(&pool),
        PATH,
        None,
        serde_json::json!({"username":"reader","password":USER_PASSWORD}),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    assert_eq!(json(login).await["user_id"], account.to_string());
    let extra_admin = db::create_account(
        &pool,
        &context(),
        db::NewAccount {
            display_name: "Extra admin",
            role: Role::Admin,
        },
    )
    .await
    .unwrap();
    assert_ne!(extra_admin, owner);
    enable_admin_password(&pool, extra_admin).await;
    db::change_account(
        &pool,
        &context(),
        owner,
        db::AccountChange::Role(Role::Writer),
    )
    .await
    .unwrap();
    assert_eq!(
        post(
            app(&pool),
            path,
            Some(&admin_cookie),
            create("other", OWNER_PASSWORD)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}
