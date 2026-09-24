use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn ambiguous_login_commit_never_returns_a_cookie_or_retries(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Reader).await;
    sqlx::raw_sql("CREATE SEQUENCE management.login_commit_attempts;
        CREATE FUNCTION management.reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('management.login_commit_attempts'); RAISE EXCEPTION 'private-detail'; END $$;
        CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON management.sessions
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION management.reject_commit();")
        .execute(&pool).await.unwrap();
    let response = login(app(&pool), &token).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(!response.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(json(response).await["error"], "outcome_unknown");
    let attempts: i64 =
        sqlx::query_scalar("SELECT last_value FROM management.login_commit_attempts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn budget_counts_invalid_tokens_and_fails_closed(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Reader).await;
    assert_eq!(
        login(app(&pool), "malformed-secret").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM management.login_budget")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 1);
    sqlx::query("UPDATE management.login_budget SET attempts=60")
        .execute(&pool)
        .await
        .unwrap();
    let response = login(app(&pool), &token).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[header::RETRY_AFTER], "60");
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    sqlx::query(
        "UPDATE management.login_budget SET window_start=clock_timestamp()-interval '2 minutes'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(login(app(&pool), &token).await.status(), StatusCode::OK);
    sqlx::query("DROP TABLE management.login_budget")
        .execute(&pool)
        .await
        .unwrap();
    let response = login(app(&pool), &token).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert_eq!(json(response).await["error"], "unavailable");
}

#[sqlx::test(migrations = "../db/migrations")]
async fn disabled_console_and_invalid_envelopes_do_not_issue_sessions(pool: PgPool) {
    let (_, _, token) = account(&pool, Role::Reader).await;
    let mut state = crate::routes::tests::test_state();
    state.pool = pool.clone();
    assert_eq!(
        login(crate::routes::app(state, &[]), &token).await.status(),
        StatusCode::NOT_FOUND
    );
    for body in [
        r#"{"token":"private-value", "role":"admin"}"#,
        r#"{"token":true}"#,
        r#"{"token":"private-value","token":"duplicate"}"#,
        r#"{"token": "#,
    ] {
        let response = request(
            app(&pool),
            "POST",
            PATH,
            &[
                ("origin", ORIGIN),
                ("x-grove-csrf", "1"),
                ("content-type", "application/json"),
            ],
            body.into(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = json(response).await;
        assert_eq!(body["error"], "invalid_input");
        assert!(!body.to_string().contains("private-value"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn mandatory_audit_failure_rolls_back_login_but_telemetry_failure_does_not(pool: PgPool) {
    let (_, id, token) = account(&pool, Role::Reader).await;
    sqlx::raw_sql("CREATE FUNCTION management.reject_login() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-db-error'; END $$;
        CREATE TRIGGER reject_login BEFORE INSERT ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_login();").execute(&pool).await.unwrap();
    let response = login(app(&pool), &token).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(
        !json(response)
            .await
            .to_string()
            .contains("private-db-error")
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let unused: bool =
        sqlx::query_scalar("SELECT last_used_at IS NULL FROM management.credentials WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(unused);
    sqlx::raw_sql("DROP TRIGGER reject_login ON management.audit_events; DROP TABLE management.command_invocations; DROP TABLE management.security_events;").execute(&pool).await.unwrap();
    assert_eq!(login(app(&pool), &token).await.status(), StatusCode::OK);
}
