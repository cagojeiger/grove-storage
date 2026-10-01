use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn mandatory_audit_failure_rolls_back_every_resource_mutation(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = audit_count(&pool).await;
    reject_audit(&pool).await;
    for command in mutations() {
        let name = command.name();
        let error = execute(&pool, &admin.token, command)
            .await
            .result
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Unavailable, "{name:?}");
        assert_eq!(error.outcome, Outcome::NotApplied);
        assert_eq!(resource_counts(&pool).await, (1, 1, 1));
        assert_eq!(audit_count(&pool).await, before);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn commit_failure_is_unknown_never_retried_and_never_returns_secret(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql("CREATE SEQUENCE public.commit_attempts;
        CREATE FUNCTION public.reject_credential_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('public.commit_attempts'); RAISE EXCEPTION 'private-db-detail'; END $$;
        CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON s3_credentials
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.reject_credential_commit();")
        .execute(&pool).await.unwrap();
    let before = audit_count(&pool).await;
    let execution = execute(
        &pool,
        &admin.token,
        Command::CredentialCreate(input::ClientInput {
            client_id: "app".into(),
        }),
    )
    .await;
    let error = execution.result.unwrap_err();
    assert_eq!(error.code, ErrorCode::Unavailable);
    assert_eq!(error.outcome, Outcome::Unknown);
    assert!(!format!("{error:?}").contains("private-db-detail"));
    let attempts: i64 = sqlx::query_scalar("SELECT last_value FROM public.commit_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(resource_counts(&pool).await, (1, 1, 1));
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

#[sqlx::test(migrations = "../db/migrations")]
async fn retained_files_block_deletion_and_registration_constraints_survive(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    sqlx::query("INSERT INTO files(client_id,state,declared_size) VALUES('app','pending',0)")
        .execute(&pool)
        .await
        .unwrap();
    let before = audit_count(&pool).await;
    let error = execute(
        &pool,
        &admin.token,
        Command::ClientDelete(input::ResourceInput { id: "app".into() }),
    )
    .await
    .result
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert_eq!(error.outcome, Outcome::NotApplied);
    assert_eq!(resource_counts(&pool).await, (1, 1, 1));
    assert_eq!(audit_count(&pool).await, before);
    for id in filegate_db::registry::RESERVED_CLIENT_IDS {
        let error = execute(
            &pool,
            &admin.token,
            Command::ClientCreate(input::ClientCreateInput {
                id: (*id).into(),
                storage_id: "local".into(),
            }),
        )
        .await
        .result
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
    }
    for (id, storage, code) in [
        ("app", "local", ErrorCode::Conflict),
        ("bad/name", "local", ErrorCode::InvalidInput),
        ("new", "missing", ErrorCode::InvalidInput),
    ] {
        let error = execute(
            &pool,
            &admin.token,
            Command::ClientCreate(input::ClientCreateInput {
                id: id.into(),
                storage_id: storage.into(),
            }),
        )
        .await
        .result
        .unwrap_err();
        assert_eq!(error.code, code);
    }
    for command in [
        Command::ClientKeyRegister(input::ClientKeyInput {
            client_id: "missing".into(),
            key_hash: format!("sha256:{}", "d".repeat(64)),
        }),
        Command::CredentialCreate(input::ClientInput {
            client_id: "missing".into(),
        }),
    ] {
        assert_eq!(
            execute(&pool, &admin.token, command)
                .await
                .result
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_telemetry_does_not_undo_audited_resource_mutation(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql(
        "DROP TABLE management.command_invocations; DROP TABLE management.security_events;",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before = audit_count(&pool).await;
    assert!(
        execute(
            &pool,
            &admin.token,
            Command::ClientDelete(input::ResourceInput { id: "app".into() })
        )
        .await
        .result
        .is_ok()
    );
    assert_eq!(resource_counts(&pool).await, (0, 0, 0));
    assert_eq!(audit_count(&pool).await, before + 1);
}
