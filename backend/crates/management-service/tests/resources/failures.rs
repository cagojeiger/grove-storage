use super::*;
use grove_management_command::Outcome;

#[sqlx::test(migrations = "../db/migrations")]
async fn storage_mutations_are_not_applied_and_direct_inputs_are_validated(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = audit_count(&pool).await;
    for command in [Command::StorageDelete(input::ResourceInput {
        id: "local".into(),
    })] {
        let error = resources::execute(
            &pool,
            &crypto(),
            Proof::Token(&admin.token),
            Surface::Cli,
            command,
        )
        .await
        .result
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::RequestRejected);
        assert_eq!(error.outcome, Outcome::NotApplied);
    }
    let error = resources::execute(
        &pool,
        &crypto(),
        Proof::Token(&admin.token),
        Surface::Cli,
        Command::UsageHistory(input::HistoryInput { days: 0 }),
    )
    .await
    .result
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    let result = resources::execute(
        &pool,
        &crypto(),
        Proof::Token(&admin.token),
        Surface::Cli,
        clients(),
    )
    .await
    .result
    .unwrap();
    assert!(matches!(result, Output::ClientList(ids) if ids == ["app"]));
    assert_eq!(audit_count(&pool).await, before);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn resource_database_failure_is_sanitized_and_telemetry_is_best_effort(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    sqlx::raw_sql(
        "DROP TABLE management.command_invocations; DROP TABLE management.security_events;",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        resources::execute(
            &pool,
            &crypto(),
            Proof::Token(&admin.token),
            Surface::Cli,
            clients()
        )
        .await
        .result
        .is_ok()
    );
    sqlx::query("ALTER TABLE clients RENAME TO private_database_detail")
        .execute(&pool)
        .await
        .unwrap();
    let error = resources::execute(
        &pool,
        &crypto(),
        Proof::Token(&admin.token),
        Surface::Cli,
        clients(),
    )
    .await
    .result
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unavailable);
    assert_eq!(error.outcome, Outcome::NotApplied);
    assert!(!format!("{error:?}").contains("private_database_detail"));
}
