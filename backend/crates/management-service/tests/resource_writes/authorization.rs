use super::*;

#[sqlx::test(migrations = "../db/migrations")]
async fn reader_tokens_and_wrong_surfaces_cannot_apply_mutations(pool: PgPool) {
    owner(&pool).await;
    let reader = user_login(&pool, Role::Reader, 2).await;
    seed(&pool).await;
    let automation_user = reader.account;
    db::issue_credential(&pool, &context(), automation_user, &key(&hash(500)))
        .await
        .unwrap();
    let before = audit_count(&pool).await;
    for token in [&reader.token, &hash(500)] {
        for surface in [
            Surface::Cli,
            Surface::Mcp,
            Surface::ResourceApi,
            Surface::Console,
        ] {
            for command in mutations() {
                let error = resources::execute(
                    &pool,
                    &crypto(),
                    unexpected_storage_probe,
                    Proof::Token(token),
                    surface,
                    command,
                )
                .await
                .result
                .unwrap_err();
                assert_eq!(error.code, ErrorCode::Forbidden);
                assert_eq!(error.outcome, Outcome::NotApplied);
            }
        }
    }
    assert_eq!(resource_counts(&pool).await, (1, 1, 1));
    assert_eq!(audit_count(&pool).await, before);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn queued_mutation_rechecks_revocation_and_user_demotion(pool: PgPool) {
    owner(&pool).await;
    let login = user_login(&pool, Role::Writer, 2).await;
    seed(&pool).await;
    let automation_user = login.account;
    let token = hash(500);
    let issued = db::issue_credential(&pool, &context(), automation_user, &key(&token))
        .await
        .unwrap();
    for revoke in [false, true] {
        let mut fence = pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(5139268467995599950)")
            .execute(&mut *fence)
            .await
            .unwrap();
        let task_pool = pool.clone();
        let proof = token.clone();
        let task = tokio::spawn(async move {
            execute(
                &task_pool,
                &proof,
                Command::ClientDelete(input::ResourceInput { id: "app".into() }),
            )
            .await
        });
        wait_for_identity_lock(&pool).await;
        if revoke {
            sqlx::query(
                "UPDATE management.api_tokens SET revoked_at=clock_timestamp() WHERE id=$1",
            )
            .bind(issued.id)
            .execute(&mut *fence)
            .await
            .unwrap();
        } else {
            sqlx::query("UPDATE management.accounts SET role='reader' WHERE id=$1")
                .bind(login.account)
                .execute(&mut *fence)
                .await
                .unwrap();
        }
        fence.commit().await.unwrap();
        let error = task.await.unwrap().result.unwrap_err();
        assert_eq!(
            error.code,
            if revoke {
                ErrorCode::Unauthorized
            } else {
                ErrorCode::Forbidden
            }
        );
        assert_eq!(resource_counts(&pool).await, (1, 1, 1));
    }
}
