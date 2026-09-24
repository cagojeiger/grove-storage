use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn cli_mcp_and_resource_api_share_role_results(pool: PgPool) {
    owner(&pool).await;
    seed(&pool).await;
    for (n, role) in [Role::Viewer, Role::Operator, Role::Admin]
        .into_iter()
        .enumerate()
    {
        let login = operator(&pool, role, n as u64 + 2).await;
        for surface in [Surface::Cli, Surface::Mcp, Surface::ResourceApi] {
            let result = resources::execute(&pool, Proof::Token(&login.token), surface, clients())
                .await
                .result
                .unwrap();
            assert!(matches!(result, Output::ClientList(ids) if ids == ["app"]));
            let result = resources::execute(&pool, Proof::Token(&login.token), surface, keys())
                .await
                .result;
            if role == Role::Viewer {
                assert_eq!(result.unwrap_err().code, ErrorCode::Forbidden);
            } else {
                assert!(result.is_ok());
            }
            let result =
                resources::execute(&pool, Proof::Session(&login.session), surface, clients())
                    .await
                    .result;
            assert_eq!(result.unwrap_err().code, ErrorCode::Forbidden);
        }
        assert!(
            resources::execute(
                &pool,
                Proof::Session(&login.session),
                Surface::Console,
                clients()
            )
            .await
            .result
            .is_ok()
        );
        assert_eq!(
            resources::execute(
                &pool,
                Proof::Token(&login.token),
                Surface::Console,
                clients()
            )
            .await
            .result
            .unwrap_err()
            .code,
            ErrorCode::Forbidden
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn agent_reads_obey_live_owner_role_and_revocation(pool: PgPool) {
    let owner = owner(&pool).await;
    seed(&pool).await;
    let agent = agent(&pool, owner.account).await;
    let token = hash(500);
    let credential = db::issue_credential(&pool, &context(), agent, &key(&token))
        .await
        .unwrap();
    assert!(
        resources::execute(&pool, Proof::Token(&token), Surface::Mcp, keys())
            .await
            .result
            .is_ok()
    );
    operator(&pool, Role::Admin, 2).await;
    db::change_account(
        &pool,
        &context(),
        owner.account,
        db::AccountChange::Role(Role::Viewer),
    )
    .await
    .unwrap();
    let denied = resources::execute(&pool, Proof::Token(&token), Surface::Mcp, keys()).await;
    assert_eq!(denied.result.unwrap_err().code, ErrorCode::Forbidden);
    let recorded_owner: uuid::Uuid = sqlx::query_scalar(
        "SELECT owner_user_id FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(denied.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recorded_owner, owner.account);
    assert!(
        resources::execute(&pool, Proof::Token(&token), Surface::Cli, clients())
            .await
            .result
            .is_ok()
    );
    db::revoke_credential(&pool, &context(), credential.id)
        .await
        .unwrap();
    assert_eq!(
        resources::execute(&pool, Proof::Token(&token), Surface::Cli, clients())
            .await
            .result
            .unwrap_err()
            .code,
        ErrorCode::Unauthorized
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn queued_resource_read_rechecks_role_after_identity_lock(pool: PgPool) {
    owner(&pool).await;
    seed(&pool).await;
    let login = operator(&pool, Role::Operator, 2).await;
    let mut fence = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(5139268467995599950)")
        .execute(&mut *fence)
        .await
        .unwrap();
    let task_pool = pool.clone();
    let task = tokio::spawn(async move {
        resources::execute(&task_pool, Proof::Token(&login.token), Surface::Mcp, keys()).await
    });
    wait_for_identity_lock(&pool).await;
    sqlx::query("UPDATE management.accounts SET role='viewer' WHERE id=$1")
        .bind(login.account)
        .execute(&mut *fence)
        .await
        .unwrap();
    fence.commit().await.unwrap();
    assert_eq!(
        task.await.unwrap().result.unwrap_err().code,
        ErrorCode::Forbidden
    );
}
