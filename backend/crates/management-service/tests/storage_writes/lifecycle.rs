use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn three_mutations_share_outputs_and_secret_free_audits_across_surfaces(pool: PgPool) {
    owner(&pool).await;
    let operator = operator(&pool, Role::Operator, 2).await;
    for surface in [Surface::Cli, Surface::Mcp, Surface::ResourceApi] {
        let before = audit_count(&pool).await;
        for command in [
            Command::StorageCreate(submission("local", "/private-address", 100)),
            Command::StorageReplace(submission("local", "/new-private-address", 200)),
            Command::StorageDelete(input::ResourceInput { id: "local".into() }),
        ] {
            let name = command.name();
            let execution = resources::execute(
                &pool,
                &crypto(),
                verified,
                Proof::Token(&operator.token),
                surface,
                command,
            )
            .await;
            assert_eq!(execution.result.unwrap().name(), name);
            let (audit, invocation): (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM management.audit_events WHERE request_id=$1), (SELECT count(*) FROM management.command_invocations WHERE request_id=$1)")
                .bind(execution.request_id).fetch_one(&pool).await.unwrap();
            assert_eq!((audit, invocation), (1, 1));
        }
        execute(
            &pool,
            &operator.token,
            Command::StorageDelete(input::ResourceInput { id: "local".into() }),
        )
        .await
        .result
        .unwrap();
        assert_eq!(audit_count(&pool).await, before + 3);
    }
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT row_to_json(t)::text FROM management.audit_events t WHERE action LIKE 'storage.%'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        rows.iter()
            .all(|r| !r.contains("private-address") && !r.contains(&operator.token))
    );
    let snapshots: (i64,i64,bool) = sqlx::query_as("SELECT (metadata->'before'->>'capacity_bytes')::bigint, (metadata->'after'->>'capacity_bytes')::bigint, (metadata->>'address_changed')::bool FROM management.audit_events WHERE action='storage.replace' LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(snapshots, (100, 200, true));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn references_block_delete_and_address_change_but_allow_capacity_updates(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    registry::insert_client(&pool, "app", "local")
        .await
        .unwrap();
    let deleted = execute(
        &pool,
        &admin.token,
        Command::StorageDelete(input::ResourceInput { id: "local".into() }),
    )
    .await;
    assert_eq!(deleted.result.unwrap_err().code, ErrorCode::Conflict);
    sqlx::raw_sql("WITH f AS (INSERT INTO files(client_id,state,declared_size) VALUES('app','pending',0) RETURNING id)
        INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'local',id::text FROM f;")
        .execute(&pool).await.unwrap();
    let changed = execute(
        &pool,
        &admin.token,
        Command::StorageReplace(submission("local", "/different", 200)),
    )
    .await;
    assert_eq!(changed.result.unwrap_err().code, ErrorCode::Conflict);
    let changed = execute(
        &pool,
        &admin.token,
        Command::StorageReplace(submission("local", "/fixture", 200)),
    )
    .await;
    assert!(
        matches!(changed.result.unwrap(), Output::StorageReplace(r) if r.capacity_bytes == 200)
    );
    let duplicate = execute(
        &pool,
        &admin.token,
        Command::StorageCreate(submission("local", "/fixture", 200)),
    )
    .await;
    assert_eq!(duplicate.result.unwrap_err().code, ErrorCode::Conflict);
    let invalid = execute(
        &pool,
        &admin.token,
        Command::StorageCreate(submission("bad/name", "/fixture", 200)),
    )
    .await;
    assert_eq!(invalid.result.unwrap_err().code, ErrorCode::InvalidInput);
}
