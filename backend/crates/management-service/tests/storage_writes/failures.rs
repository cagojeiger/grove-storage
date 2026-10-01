use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn audit_failure_rolls_back_all_three_storage_mutations(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = audit_count(&pool).await;
    reject_audit(&pool).await;
    for command in mutations() {
        let error = execute(&pool, &admin.token, command)
            .await
            .result
            .unwrap_err();
        assert_eq!(
            (error.code, error.outcome),
            (ErrorCode::Unavailable, Outcome::NotApplied)
        );
        let rows = registry::list_storages(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        let row = rows.first().unwrap();
        assert_eq!(
            row.endpoint.as_deref(),
            Some("https://storage.test/fixture")
        );
        assert_eq!(row.capacity_bytes, 100);
        assert_eq!(audit_count(&pool).await, before);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn commit_failure_is_unknown_and_not_retried_for_all_storage_mutations(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = audit_count(&pool).await;
    sqlx::raw_sql("CREATE SEQUENCE public.storage_attempts;
        CREATE FUNCTION public.reject_storage_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('public.storage_attempts'); RAISE EXCEPTION 'private-storage-error'; END $$;
        CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT OR UPDATE OR DELETE ON storages
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.reject_storage_commit();")
        .execute(&pool).await.unwrap();
    for (index, command) in mutations().into_iter().enumerate() {
        let execution = execute(&pool, &admin.token, command).await;
        let error = execution.result.unwrap_err();
        assert_eq!(
            (error.code, error.outcome),
            (ErrorCode::Unavailable, Outcome::Unknown)
        );
        let attempts: i64 = sqlx::query_scalar("SELECT last_value FROM public.storage_attempts")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(attempts, index as i64 + 1);
        assert_eq!(registry::list_storages(&pool).await.unwrap().len(), 1);
        assert_eq!(
            registry::get_storage(&pool, "local")
                .await
                .unwrap()
                .unwrap()
                .capacity_bytes,
            100
        );
        assert_eq!(audit_count(&pool).await, before);
        let outcome: String = sqlx::query_scalar(
            "SELECT outcome FROM management.command_invocations WHERE request_id=$1",
        )
        .bind(execution.request_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(outcome, "unknown");
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_probe_is_not_applied_and_missing_replace_skips_probe(pool: PgPool) {
    let admin = owner(&pool).await;
    let before = audit_count(&pool).await;
    let missing = resources::execute(
        &pool,
        &crypto(),
        unexpected_storage_probe,
        Proof::Token(&admin.token),
        Surface::Cli,
        Command::StorageReplace(submission("missing", "/fixture", 1)),
    )
    .await;
    assert_eq!(missing.result.unwrap_err().code, ErrorCode::NotFound);
    let failed = resources::execute(
        &pool,
        &crypto(),
        |_| async { Err(Error::InvalidInput) },
        Proof::Token(&admin.token),
        Surface::Cli,
        Command::StorageCreate(submission("new", "/fixture", 1)),
    )
    .await;
    assert_eq!(failed.result.unwrap_err().outcome, Outcome::NotApplied);
    assert!(registry::list_storages(&pool).await.unwrap().is_empty());
    assert_eq!(audit_count(&pool).await, before);
}
