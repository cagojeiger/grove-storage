use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn password_budgets_isolate_accounts_and_authenticated_operations(pool: PgPool) {
    let (alice, _, _) = account(&pool, Role::Admin).await;
    let (bob, _, _) = account(&pool, Role::Reader).await;
    for (id, name) in [(alice, "alice"), (bob, "bob")] {
        grove_management_service::local_accounts::recover(
            &pool,
            Uuid::new_v4(),
            id,
            name,
            PASSWORD.into(),
        )
        .await
        .unwrap();
    }
    let response = sign_in(app(&pool), "alice", PASSWORD).await;
    assert_eq!(response.status(), StatusCode::OK);
    let alice_cookie = cookie(&response);
    sqlx::query("UPDATE management.authentication_budgets SET attempts=59 WHERE account_id=$1 AND purpose='login'")
        .bind(alice).execute(&pool).await.unwrap();
    assert_eq!(
        sign_in(app(&pool), " ALICE ", "wrong").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let limited = sign_in(app(&pool), "alice", PASSWORD).await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.headers()[header::RETRY_AFTER], "60");
    assert!(!limited.headers().contains_key(header::SET_COOKIE));
    assert_eq!(
        sign_in(app(&pool), "bob", PASSWORD).await.status(),
        StatusCode::OK
    );

    sqlx::query("UPDATE management.login_budget SET attempts=60")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sign_in(app(&pool), "unknown", "wrong").await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        sign_in(app(&pool), "bob", PASSWORD).await.status(),
        StatusCode::OK
    );
    // Same-password rejection proves public login exhaustion does not block reauthentication.
    assert_eq!(
        change_password(app(&pool), &alice_cookie, PASSWORD, PASSWORD)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    sqlx::query("UPDATE management.authentication_budgets SET attempts=60 WHERE account_id=$1 AND purpose='reauthentication'")
        .bind(alice).execute(&pool).await.unwrap();
    let replacement = "another private phrase for admission testing";
    assert_eq!(
        change_password(app(&pool), &alice_cookie, PASSWORD, replacement)
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    let issue = request(
        app(&pool),
        "POST",
        &format!("/api/admin/identity/v1/accounts/{alice}/credentials"),
        &[
            ("cookie", &alice_cookie),
            ("origin", ORIGIN),
            ("x-grove-csrf", "1"),
            ("content-type", "application/json"),
        ],
        serde_json::json!({"label":"blocked", "current_password":PASSWORD}).to_string(),
    )
    .await;
    assert_eq!(issue.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        current(app(&pool), &alice_cookie).await.status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE management.authentication_budgets SET window_start=clock_timestamp()-interval '2 minutes' WHERE purpose='login'")
        .execute(&pool).await.unwrap();
    assert_eq!(
        sign_in(app(&pool), "alice", PASSWORD).await.status(),
        StatusCode::OK
    );
    sqlx::query("UPDATE management.authentication_budgets SET window_start=clock_timestamp()-interval '2 minutes' WHERE purpose='reauthentication'")
        .execute(&pool).await.unwrap();
    assert_eq!(
        change_password(app(&pool), &alice_cookie, PASSWORD, replacement)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
}
