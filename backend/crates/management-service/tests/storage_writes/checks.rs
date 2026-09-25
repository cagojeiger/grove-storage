use super::*;
use resources::StorageOperation;
use std::time::Duration;

fn check(id: &str) -> Command {
    Command::StorageTest(input::ResourceInput { id: id.into() })
}

async fn connected(operation: StorageOperation) -> Result<StorageRow, Error> {
    match operation {
        StorageOperation::Test(row) => Ok(row),
        _ => Err(Error::InvalidInput),
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn reader_tests_saved_storage_across_surfaces_without_registry_or_audit_mutations(
    pool: PgPool,
) {
    owner(&pool).await;
    let reader = user_login(&pool, Role::Reader, 2).await;
    seed(&pool).await;
    let before = registry::get_storage(&pool, "local")
        .await
        .unwrap()
        .unwrap();
    let audits = audit_count(&pool).await;
    for surface in [
        Surface::Cli,
        Surface::Mcp,
        Surface::ResourceApi,
        Surface::Console,
    ] {
        let proof = if surface == Surface::Console {
            Proof::Session(&reader.session)
        } else {
            Proof::Token(&reader.token)
        };
        let execution =
            resources::execute(&pool, &crypto(), connected, proof, surface, check("local")).await;
        assert!(
            matches!(execution.result.unwrap(), Output::StorageTest(r) if r.id == "local" && r.state == grove_management_command::model::State::Ok)
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM management.command_invocations WHERE request_id=$1 AND operation='storage.test' AND outcome='succeeded'")
            .bind(execution.request_id).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }
    assert_eq!(audit_count(&pool).await, audits);
    assert_eq!(
        registry::get_storage(&pool, "local")
            .await
            .unwrap()
            .unwrap(),
        before
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn missing_unsupported_and_unauthenticated_checks_never_probe(pool: PgPool) {
    let admin = owner(&pool).await;
    sqlx::query("INSERT INTO storages(id,kind,root_path,capacity_bytes) VALUES('legacy','fs','/never-touch',1)")
        .execute(&pool).await.unwrap();
    for (id, token, expected) in [
        ("missing", admin.token.as_str(), ErrorCode::NotFound),
        ("legacy", admin.token.as_str(), ErrorCode::InvalidInput),
        ("legacy", "invalid", ErrorCode::Unauthorized),
    ] {
        let result = resources::execute(
            &pool,
            &crypto(),
            unexpected_storage_probe,
            Proof::Token(token),
            Surface::Cli,
            check(id),
        )
        .await;
        assert_eq!(result.result.unwrap_err().code, expected);
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn saved_setting_changes_and_revocation_during_probe_reject_success(pool: PgPool) {
    owner(&pool).await;
    seed(&pool).await;
    for revoke in [false, true] {
        let user = user_login(&pool, Role::Reader, if revoke { 3 } else { 2 }).await;
        let probe = |operation| async {
            tokio::time::timeout(Duration::from_secs(3), async {
                if revoke {
                    db::revoke_credential(&pool, &context(), user.credential)
                        .await
                        .unwrap();
                } else {
                    sqlx::query(
                        "UPDATE storages SET capacity_bytes=capacity_bytes+1 WHERE id='local'",
                    )
                    .execute(&pool)
                    .await
                    .unwrap();
                }
            })
            .await
            .unwrap();
            connected(operation).await
        };
        let result = resources::execute(
            &pool,
            &crypto(),
            probe,
            Proof::Token(&user.token),
            Surface::Cli,
            check("local"),
        )
        .await;
        assert_eq!(
            result.result.unwrap_err().code,
            if revoke {
                ErrorCode::Unauthorized
            } else {
                ErrorCode::Conflict
            }
        );
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn failed_and_timed_out_probes_preserve_registry(pool: PgPool) {
    let admin = owner(&pool).await;
    seed(&pool).await;
    let before = registry::get_storage(&pool, "local")
        .await
        .unwrap()
        .unwrap();
    for stall in [false, true] {
        let probe = |_| async move {
            if stall {
                std::future::pending::<()>().await;
            }
            Err(Error::Unavailable)
        };
        let result = resources::execute(
            &pool,
            &crypto(),
            probe,
            Proof::Token(&admin.token),
            Surface::Cli,
            check("local"),
        )
        .await;
        let error = result.result.unwrap_err();
        assert_eq!(error.code, ErrorCode::Unavailable);
        assert_eq!(error.outcome, Outcome::NotApplied);
    }
    assert_eq!(
        registry::get_storage(&pool, "local")
            .await
            .unwrap()
            .unwrap(),
        before
    );
}
