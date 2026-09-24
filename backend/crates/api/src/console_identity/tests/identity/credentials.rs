use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn tokens_are_one_time_responses_and_revocation_invalidates_sessions(pool: PgPool) {
    let (_, admin_cookie) = actor(&pool, Role::Admin).await;
    let user = create(
        &pool,
        &admin_cookie,
        serde_json::json!({"kind":"user","display_name":"target","role":"reader"}),
    )
    .await;
    let key = issue(&pool, &admin_cookie, user).await;
    let raw = key["token"].as_str().unwrap();
    let response = login(app(&pool), raw).await;
    assert_eq!(response.status(), StatusCode::OK);
    let user_cookie = cookie(&response);
    let list = get(
        &pool,
        &admin_cookie,
        &format!("/accounts/{user}/credentials"),
    )
    .await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains(raw));
    assert!(!list.to_string().contains("token_hash"));
    let expire = chrono::DateTime::parse_from_rfc3339(key["expires_at"].as_str().unwrap()).unwrap();
    assert!(expire < Utc::now() + Duration::days(2));
    for table in ["audit_events", "command_invocations", "security_events"] {
        let rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT row_to_json(t)::text FROM management.{table} t"
        ))
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(
            rows.iter()
                .all(|row| !row.contains(raw) && !row.contains(&secrets::token_hash(raw)))
        );
    }
    let response = send(
        &pool,
        &admin_cookie,
        "DELETE",
        &format!("/credentials/{}", key["credential_id"].as_str().unwrap()),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        current(app(&pool), &user_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for days in [0, 91] {
        assert_eq!(
            send(
                &pool,
                &admin_cookie,
                "POST",
                &format!("/accounts/{user}/credentials"),
                serde_json::json!({"label":"invalid","expires_in_days":days})
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn user_deletion_retires_all_credentials_and_session_scope_is_self_only(pool: PgPool) {
    let (_, admin_cookie) = actor(&pool, Role::Admin).await;
    let (user, user_cookie) = actor(&pool, Role::Reader).await;
    let key = issue(&pool, &admin_cookie, user).await;
    let raw = key["token"].as_str().unwrap();
    let second_login = login(app(&pool), raw).await;
    assert_eq!(second_login.status(), StatusCode::OK);
    let identity = db::authenticate(&pool, &secrets::token_hash(raw))
        .await
        .unwrap()
        .unwrap();
    assert!(
        grove_management_policy::authorize(
            identity.caller,
            Surface::Cli,
            grove_management_policy::Action::WriteResources
        )
        .is_err()
    );
    let admin_sessions = get(&pool, &admin_cookie, "/sessions").await;
    let admin_session = admin_sessions["items"][0]["id"].as_str().unwrap();
    let response = send(
        &pool,
        &user_cookie,
        "DELETE",
        &format!("/sessions/{admin_session}"),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(json(response).await["changed"], false);
    assert_eq!(
        current(app(&pool), &admin_cookie).await.status(),
        StatusCode::OK
    );
    let own = get(&pool, &user_cookie, "/sessions").await;
    assert_eq!(own["items"].as_array().unwrap().len(), 2);
    let current_id = json(current(app(&pool), &user_cookie).await).await["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = send(
        &pool,
        &user_cookie,
        "DELETE",
        &format!("/sessions/{}", current_id),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        current(app(&pool), &user_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &pool,
            &admin_cookie,
            "DELETE",
            &format!("/accounts/{user}"),
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert!(
        db::authenticate(&pool, &secrets::token_hash(raw))
            .await
            .unwrap()
            .is_none()
    );
}
