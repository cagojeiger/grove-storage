#![allow(clippy::unwrap_used)]
mod support;
use filegate_db::{
    PgPool,
    management::{self as db, AccountChange, NewAccount},
};
use grove_management_policy::{Role, Surface};
use grove_management_service::{self as service, Command, Error, Output, Page, Proof};
use support::*;
use uuid::Uuid;

fn commands(target: Uuid, key_hash: &str) -> Vec<(Command<'_>, bool)> {
    vec![
        (
            Command::CreateAccount(NewAccount {
                display_name: "created",
                role: Role::Reader,
            }),
            true,
        ),
        (
            Command::ChangeAccount {
                id: target,
                change: AccountChange::Role(Role::Reader),
            },
            true,
        ),
        (
            Command::IssueCredential {
                account: target,
                key: key(key_hash),
            },
            true,
        ),
        (Command::RevokeCredential(Uuid::new_v4()), true),
        (Command::Accounts(Page::default().into()), true),
        (
            Command::Credentials {
                account: target,
                page: Page::default(),
            },
            true,
        ),
        (Command::Security(Page::default()), true),
        (Command::OwnSessions(Page::default()), false),
        (Command::CurrentSession, false),
        (Command::RevokeOwnSession(Uuid::new_v4()), false),
        (Command::Audit(Page::default()), false),
        (Command::Invocations(Page::default()), false),
        (Command::Logout, false),
    ]
}

#[test]
fn identity_and_history_names_never_overlap_resource_commands() {
    let target = Uuid::new_v4();
    let hash = hash(99);
    let mut cases = commands(target, &hash);
    cases.push((
        Command::ChangeAccount {
            id: target,
            change: AccountChange::Active(false),
        },
        true,
    ));
    cases.push((
        Command::ChangeAccount {
            id: target,
            change: AccountChange::Delete,
        },
        true,
    ));
    for (command, _) in cases {
        assert!(command.name().starts_with("identity.") || command.name().starts_with("history."));
        assert!(grove_management_command::CommandName::parse(command.name()).is_none());
    }
    assert_eq!(
        Command::Logout.action(),
        grove_management_policy::Action::RevokeOwnSessions
    );
    assert!(grove_management_command::CommandName::parse(Command::Logout.name()).is_none());
}

#[sqlx::test(migrations = "../db/migrations")]
async fn every_console_operation_enforces_its_role(pool: PgPool) {
    owner(&pool).await;
    let target = user(&pool, Role::Reader).await;
    let key_hash = hash(99);
    for (n, role) in [Role::Reader, Role::Writer, Role::Admin]
        .into_iter()
        .enumerate()
    {
        let login = user_login(&pool, role, n as u64 + 2).await;
        for (command, admin_only) in commands(target, &key_hash) {
            let name = command.name();
            let result = service::execute(
                &pool,
                Proof::Session(&login.session),
                Surface::Console,
                command,
            )
            .await
            .result;
            if admin_only && role != Role::Admin {
                assert!(
                    matches!(result, Err(Error::Forbidden)),
                    "{name}: {result:?}"
                );
            } else {
                assert!(result.is_ok(), "{name}: {result:?}");
            }
        }
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn admin_bearer_cannot_use_any_console_command_or_fake_its_surface(pool: PgPool) {
    let login = owner(&pool).await;
    let key_hash = hash(99);
    for surface in [
        Surface::Cli,
        Surface::Mcp,
        Surface::ResourceApi,
        Surface::Console,
    ] {
        for (command, _) in commands(login.account, &key_hash) {
            assert!(matches!(
                service::execute(&pool, Proof::Token(&login.token), surface, command)
                    .await
                    .result,
                Err(Error::Forbidden)
            ));
        }
    }
    assert!(matches!(
        service::execute(
            &pool,
            Proof::Session(&login.session),
            Surface::Cli,
            Command::Accounts(Page::default().into())
        )
        .await
        .result,
        Err(Error::Forbidden)
    ));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn own_session_scope_and_current_revocation_are_enforced(pool: PgPool) {
    let admin = owner(&pool).await;
    let reader = user_login(&pool, Role::Reader, 2).await;
    let result = service::execute(
        &pool,
        Proof::Session(&reader.session),
        Surface::Console,
        Command::OwnSessions(Page::default()),
    )
    .await
    .result
    .unwrap();
    assert!(
        matches!(result, Output::Sessions(ref rows) if rows.len()==1 && rows.first().unwrap().id==reader.session_id)
    );
    let result = service::execute(
        &pool,
        Proof::Session(&reader.session),
        Surface::Console,
        Command::RevokeOwnSession(admin.session_id),
    )
    .await
    .result
    .unwrap();
    assert!(matches!(result, Output::Changed(false)));
    assert!(
        db::session_actor(&pool, &admin.session)
            .await
            .unwrap()
            .is_some()
    );
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::RevokeCredential(reader.credential),
    )
    .await
    .result
    .unwrap();
    assert!(matches!(result, Output::Changed(true)));
    assert!(matches!(
        service::execute(
            &pool,
            Proof::Session(&reader.session),
            Surface::Console,
            Command::OwnSessions(Page::default())
        )
        .await
        .result,
        Err(Error::Unauthenticated)
    ));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn waiting_mutation_rechecks_role_after_lock_acquisition(pool: PgPool) {
    owner(&pool).await;
    let login = user_login(&pool, Role::Admin, 2).await;
    let mut fence = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(5139268467995599950)")
        .execute(&mut *fence)
        .await
        .unwrap();
    let task_pool = pool.clone();
    let task = tokio::spawn(async move {
        service::execute(
            &task_pool,
            Proof::Session(&login.session),
            Surface::Console,
            Command::CreateAccount(NewAccount {
                display_name: "stale-authority",
                role: Role::Admin,
            }),
        )
        .await
    });
    wait_for_identity_lock(&pool).await;
    sqlx::query("UPDATE management.accounts SET role='reader' WHERE id=$1")
        .bind(login.account)
        .execute(&mut *fence)
        .await
        .unwrap();
    fence.commit().await.unwrap();
    assert!(matches!(task.await.unwrap().result, Err(Error::Forbidden)));
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM management.accounts WHERE display_name='stale-authority')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!exists);
}

#[sqlx::test(migrations = "../db/migrations")]
async fn waiting_mutation_rechecks_credential_after_lock_acquisition(pool: PgPool) {
    let login = owner(&pool).await;
    let mut fence = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(5139268467995599950)")
        .execute(&mut *fence)
        .await
        .unwrap();
    let task_pool = pool.clone();
    let task = tokio::spawn(async move {
        service::execute(
            &task_pool,
            Proof::Session(&login.session),
            Surface::Console,
            Command::Accounts(Page::default().into()),
        )
        .await
    });
    wait_for_identity_lock(&pool).await;
    sqlx::query("UPDATE management.credentials SET revoked_at=clock_timestamp() WHERE id=$1")
        .bind(login.credential)
        .execute(&mut *fence)
        .await
        .unwrap();
    fence.commit().await.unwrap();
    assert!(matches!(
        task.await.unwrap().result,
        Err(Error::Unauthenticated)
    ));
}
