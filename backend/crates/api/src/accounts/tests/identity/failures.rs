use super::*;

async fn rejected_issue(pool: &PgPool, cookie: &str, user: Uuid, error: &str) {
    let response = send(
        pool,
        cookie,
        "POST",
        &format!("/accounts/{user}/credentials"),
        serde_json::json!({"label":"private-label","current_password":PASSWORD}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key(header::RETRY_AFTER));
    let body = json(response).await;
    assert_eq!(body["error"], error);
    assert!(body.get("token").is_none());
    assert!(!body.to_string().contains("private-"));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM management.api_tokens WHERE account_id=$1")
            .bind(user)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_issuance_audit_rolls_back_without_delivering_a_token(pool: PgPool) {
    let (user, cookie) = actor(&pool, Role::Admin).await;
    sqlx::raw_sql("CREATE FUNCTION management.reject_issue() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-db-detail'; END $$;
        CREATE TRIGGER reject_issue BEFORE INSERT ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_issue();")
        .execute(&pool).await.unwrap();
    rejected_issue(&pool, &cookie, user, "unavailable").await;
}

#[sqlx::test(migrations = "../db/migrations")]
async fn uncertain_issuance_commit_never_delivers_a_token_or_retries(pool: PgPool) {
    let (user, cookie) = actor(&pool, Role::Admin).await;
    sqlx::raw_sql(
        "CREATE SEQUENCE management.issue_attempts;
        CREATE FUNCTION management.reject_issue() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('management.issue_attempts'); RAISE EXCEPTION 'private-db-detail'; END $$;
        CREATE CONSTRAINT TRIGGER reject_issue AFTER INSERT ON management.api_tokens
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION management.reject_issue();",
    )
    .execute(&pool)
    .await
    .unwrap();
    rejected_issue(&pool, &cookie, user, "outcome_unknown").await;
    let attempts: i64 = sqlx::query_scalar("SELECT last_value FROM management.issue_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 1);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn issuance_limit_returns_conflict_without_a_secret(pool: PgPool) {
    let (user, cookie) = actor(&pool, Role::Admin).await;
    for _ in 0..32 {
        credential(&pool, user).await;
    }
    let response = send(
        &pool,
        &cookie,
        "POST",
        &format!("/accounts/{user}/credentials"),
        serde_json::json!({"label":"over-limit","current_password":PASSWORD}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = json(response).await;
    assert_eq!(body["error"], "conflict");
    assert!(body.get("token").is_none());
}
