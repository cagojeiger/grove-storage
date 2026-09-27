use super::*;

const PASSWORD: &str = "a private phrase for personal token tests";

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

#[sqlx::test(migrations = "../db/migrations")]
async fn personal_tokens_require_password_session_and_cannot_cross_accounts(pool: PgPool) {
    let owner = grove_management_service::local_accounts::initialize(
        &pool,
        Uuid::new_v4(),
        "owner",
        "Owner",
        PASSWORD.into(),
    )
    .await
    .unwrap();
    let signed_in = post(
        app(&pool),
        PATH,
        None,
        serde_json::json!({"username":"owner","password":PASSWORD}),
    )
    .await;
    assert_eq!(signed_in.status(), StatusCode::OK);
    let own_cookie = cookie(&signed_in);
    let path = "/api/admin/identity/v1/me/tokens";
    let input = |password: &str| {
        serde_json::json!({
            "label":"CLI laptop", "expires_in_days":7, "current_password":password,
        })
    };
    assert_eq!(
        post(app(&pool), path, None, input(PASSWORD)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(app(&pool), path, Some(&own_cookie), input("wrong"))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let no_csrf = request(
        app(&pool),
        "POST",
        path,
        &[("origin", ORIGIN), ("cookie", &own_cookie)],
        input(PASSWORD).to_string(),
    )
    .await;
    assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);
    let issued = post(app(&pool), path, Some(&own_cookie), input(PASSWORD)).await;
    assert_eq!(issued.status(), StatusCode::CREATED);
    let issued = json(issued).await;
    let raw = issued["token"].as_str().unwrap();
    let token_id: Uuid = issued["credential_id"].as_str().unwrap().parse().unwrap();
    assert_eq!(issued["account_id"], owner.to_string());
    assert!(secrets::valid(raw, secrets::TOKEN_PREFIX));
    let list = request(
        app(&pool),
        "GET",
        path,
        &[("cookie", &own_cookie)],
        String::new(),
    )
    .await;
    assert_eq!(list.status(), StatusCode::OK);
    let list = json(list).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["id"], token_id.to_string());
    assert!(!list.to_string().contains(raw));
    let other = db::create_account(
        &pool,
        &context(),
        db::NewAccount {
            display_name: "Other",
            role: Role::Reader,
        },
    )
    .await
    .unwrap();
    let (other_token, _) = credential(&pool, other).await;
    let revoke_other = request(
        app(&pool),
        "DELETE",
        &format!("{path}/{other_token}"),
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("cookie", &own_cookie),
        ],
        String::new(),
    )
    .await;
    assert_eq!(revoke_other.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        login(app(&pool), raw).await.status(),
        StatusCode::BAD_REQUEST
    );
    let revoke_own = request(
        app(&pool),
        "DELETE",
        &format!("{path}/{token_id}"),
        &[
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("cookie", &own_cookie),
        ],
        String::new(),
    )
    .await;
    assert_eq!(revoke_own.status(), StatusCode::OK);
    assert_eq!(json(revoke_own).await["changed"], true);
    assert!(
        db::authenticate(&pool, &secrets::token_hash(raw))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        request(
            app(&pool),
            "GET",
            path,
            &[("cookie", &own_cookie)],
            String::new()
        )
        .await
        .status(),
        StatusCode::OK
    );
}
