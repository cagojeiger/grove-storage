use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_audit_rolls_back_issuance_and_unknown_commit_is_not_retried(pool: PgPool) {
    let token = owner(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql("CREATE FUNCTION management.reject_mcp_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private-mcp-detail'; END $$;
        CREATE TRIGGER reject_mcp_audit BEFORE INSERT ON management.audit_events FOR EACH ROW EXECUTE FUNCTION management.reject_mcp_audit();")
        .execute(&pool).await.unwrap();
    let failure = call(
        &pool,
        &token,
        "credential.create",
        json!({"client_id":"app"}),
    )
    .await;
    assert_eq!(failure["isError"], true);
    assert_eq!(
        failure["structuredContent"]["error"],
        json!({"code":"unavailable","outcome":"not_applied"})
    );
    assert!(!failure.to_string().contains("private-mcp-detail"));
    assert!(failure["structuredContent"].get("result").is_none());
    sqlx::raw_sql(
        "DROP TRIGGER reject_mcp_audit ON management.audit_events;
        CREATE SEQUENCE public.mcp_commit_attempts;
        CREATE FUNCTION public.reject_mcp_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('public.mcp_commit_attempts'); RAISE EXCEPTION 'private-mcp-detail'; END $$;
        CREATE CONSTRAINT TRIGGER reject_mcp_commit AFTER INSERT ON client_s3_credentials
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.reject_mcp_commit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let failure = call(
        &pool,
        &token,
        "credential.create",
        json!({"client_id":"app"}),
    )
    .await;
    assert_eq!(failure["isError"], true);
    assert_eq!(
        failure["structuredContent"]["error"],
        json!({"code":"unavailable","outcome":"unknown"})
    );
    assert!(!failure.to_string().contains("private-mcp-detail"));
    assert!(failure["structuredContent"].get("result").is_none());
    let attempts: i64 = sqlx::query_scalar("SELECT last_value FROM public.mcp_commit_attempts")
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
