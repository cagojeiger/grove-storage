use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn resource_audit_failure_returns_no_secret_and_restores_database(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql("CREATE FUNCTION management.reject_resource_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-detail'; END $$;
        CREATE TRIGGER reject_resource_audit BEFORE INSERT ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_resource_audit();")
        .execute(&pool).await.unwrap();
    let response = call(
        &pool,
        &token,
        "credential.create",
        json!({"client_id":"app"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json_body(response).await;
    assert_eq!(
        body["error"],
        json!({"code":"unavailable","outcome":"not_applied"})
    );
    assert!(body.get("result").is_none());
    assert!(!body.to_string().contains("private-detail"));
    assert_eq!(
        grove_db::s3_registry::list_credentials(&pool, "app")
            .await
            .unwrap(),
        ["testaccesskey"]
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn uncertain_resource_commit_has_unknown_outcome_without_secret_or_retry(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql("CREATE SEQUENCE public.resource_commit_attempts;
        CREATE FUNCTION public.reject_resource_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('public.resource_commit_attempts'); RAISE EXCEPTION 'private-detail'; END $$;
        CREATE CONSTRAINT TRIGGER reject_resource_commit AFTER INSERT ON client_s3_credentials
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.reject_resource_commit();")
        .execute(&pool).await.unwrap();
    let response = call(
        &pool,
        &token,
        "credential.create",
        json!({"client_id":"app"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key(header::RETRY_AFTER));
    let body = json_body(response).await;
    assert_eq!(
        body["error"],
        json!({"code":"unavailable","outcome":"unknown"})
    );
    assert!(body.get("result").is_none());
    assert!(!body.to_string().contains("private-detail"));
    let attempts: i64 =
        sqlx::query_scalar("SELECT last_value FROM public.resource_commit_attempts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(
        grove_db::s3_registry::list_credentials(&pool, "app")
            .await
            .unwrap(),
        ["testaccesskey"]
    );
}
