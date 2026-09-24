use super::*;
use std::time::Duration;

#[sqlx::test(migrations = "../db/migrations")]
async fn denied_callers_never_probe_or_modify_storage(pool: PgPool) {
    owner(&pool).await;
    let viewer = operator(&pool, Role::Viewer, 2).await;
    let agent = agent(&pool, viewer.account).await;
    db::issue_credential(&pool, &context(), agent, &key(&hash(500)))
        .await
        .unwrap();
    seed(&pool).await;
    let before = audit_count(&pool).await;
    for token in [&viewer.token, &hash(500)] {
        for command in mutations() {
            let result = resources::execute(
                &pool,
                &crypto(),
                unexpected_storage_probe,
                Proof::Token(token),
                Surface::Mcp,
                command,
            )
            .await;
            assert_eq!(result.result.unwrap_err().code, ErrorCode::Forbidden);
        }
    }
    assert_eq!(audit_count(&pool).await, before);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn probe_releases_identity_fence_and_rechecks_owner_role_and_revocation(pool: PgPool) {
    owner(&pool).await;
    for revoke in [false, true] {
        let user = operator(&pool, Role::Operator, if revoke { 3 } else { 2 }).await;
        let agent = agent(&pool, user.account).await;
        let token = hash(if revoke { 501 } else { 500 });
        let key = db::issue_credential(&pool, &context(), agent, &key(&token))
            .await
            .unwrap();
        let verify = |input| async {
            // This mutation takes the same advisory lock. A held probe lock would time out.
            tokio::time::timeout(Duration::from_secs(3), async {
                if revoke {
                    db::revoke_credential(&pool, &context(), key.id)
                        .await
                        .unwrap();
                } else {
                    db::change_account(
                        &pool,
                        &context(),
                        user.account,
                        db::AccountChange::Role(Role::Viewer),
                    )
                    .await
                    .unwrap();
                }
            })
            .await
            .unwrap();
            verified(input).await
        };
        let execution = resources::execute(
            &pool,
            &crypto(),
            verify,
            Proof::Token(&token),
            Surface::Mcp,
            Command::StorageCreate(submission("new", "/fixture", 1)),
        )
        .await;
        assert_eq!(
            execution.result.unwrap_err().code,
            if revoke {
                ErrorCode::Unauthorized
            } else {
                ErrorCode::Forbidden
            }
        );
        assert!(registry::list_storages(&pool).await.unwrap().is_empty());
        let audits: i64 =
            sqlx::query_scalar("SELECT count(*) FROM management.audit_events WHERE request_id=$1")
                .bind(execution.request_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(audits, 0);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn replacement_rechecks_resource_state_after_probe(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    registry::insert_client(&pool, "app", "local")
        .await
        .unwrap();
    let verify = |input| async {
        sqlx::raw_sql("WITH f AS (INSERT INTO files(client_id,state,declared_size) VALUES('app','pending',0) RETURNING id)
            INSERT INTO locations(file_id,storage_id,object_key) SELECT id,'local',id::text FROM f;")
            .execute(&pool).await.unwrap();
        verified(input).await
    };
    let execution = resources::execute(
        &pool,
        &crypto(),
        verify,
        Proof::Token(&admin.token),
        Surface::Cli,
        Command::StorageReplace(submission("local", "/new", 200)),
    )
    .await;
    assert_eq!(execution.result.unwrap_err().code, ErrorCode::Conflict);
    assert_eq!(
        registry::get_storage(&pool, "local")
            .await
            .unwrap()
            .unwrap()
            .endpoint
            .as_deref(),
        Some("https://storage.test/fixture")
    );
    let verify = |input| async {
        sqlx::query("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('new','fs','/concurrent',1)")
            .execute(&pool).await.unwrap();
        verified(input).await
    };
    let execution = resources::execute(
        &pool,
        &crypto(),
        verify,
        Proof::Token(&admin.token),
        Surface::Cli,
        Command::StorageCreate(submission("new", "/fixture", 1)),
    )
    .await;
    assert_eq!(execution.result.unwrap_err().code, ErrorCode::Conflict);
}
