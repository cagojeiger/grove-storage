#![allow(clippy::unwrap_used, clippy::unreachable)]
mod support;
use grove_db::{
    PgPool,
    management::{self as db, AuditActor, AuditContext, NewAccount, telemetry},
};
use grove_management_policy::{Role, Surface};
use grove_management_service::{self as service, Command, Error, Output, Page, Proof};
use support::*;
use uuid::Uuid;

#[sqlx::test(migrations = "../db/migrations")]
async fn history_scope_uses_actor_snapshots_and_filters_before_pagination(pool: PgPool) {
    let admin = owner(&pool).await;
    let reader = user_login(&pool, Role::Reader, 2).await;
    let account_token = db::issue_credential(&pool, &context(), reader.account, &key(&hash(3)))
        .await
        .unwrap();
    let ctx = AuditContext {
        actor: AuditActor::Account {
            id: reader.account,
            session_id: None,
            credential_id: Some(account_token.id),
        },
        request_id: Uuid::new_v4(),
        surface: Surface::Mcp,
    };
    // Seed a prior operation; transport authorization is exercised by the service tests.
    db::create_account(
        &pool,
        &ctx,
        NewAccount {
            display_name: "historic-target",
            role: Role::Reader,
        },
    )
    .await
    .unwrap();
    telemetry::invocation(
        &pool,
        &ctx,
        "storage.list",
        telemetry::Outcome::Succeeded,
        None,
        1,
    )
    .await
    .unwrap();
    let foreign = context();
    db::create_account(
        &pool,
        &foreign,
        NewAccount {
            display_name: "foreign-target",
            role: Role::Reader,
        },
    )
    .await
    .unwrap();
    let result = service::execute(
        &pool,
        Proof::Session(&reader.session),
        Surface::Console,
        Command::Audit(Page::new(None, 1).unwrap().into()),
    )
    .await
    .result
    .unwrap();
    let first = match result {
        Output::Audit(rows) => rows,
        _ => unreachable!(),
    };
    assert_eq!(first.len(), 1);
    assert_eq!(first.first().unwrap().context.request_id, ctx.request_id);
    let cursor = first.first().unwrap().context.id;
    let result = service::execute(
        &pool,
        Proof::Session(&reader.session),
        Surface::Console,
        Command::Audit(Page::new(Some(cursor), 100).unwrap().into()),
    )
    .await
    .result
    .unwrap();
    let rest = match result {
        Output::Audit(rows) => rows,
        _ => unreachable!(),
    };
    assert!(!rest.is_empty());
    assert!(
        rest.iter()
            .all(|row| row.context.id < cursor && row.context.actor_id == Some(reader.account))
    );
    let result = service::execute(
        &pool,
        Proof::Session(&reader.session),
        Surface::Console,
        Command::Invocations(Page::default().into()),
    )
    .await
    .result
    .unwrap();
    let rows = match result {
        Output::Invocations(rows) => rows,
        _ => unreachable!(),
    };
    assert!(
        rows.iter()
            .any(|row| row.context.request_id == ctx.request_id)
    );
    assert!(
        rows.iter()
            .all(|row| row.context.actor_id == Some(reader.account))
    );
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Audit(Page::default().into()),
    )
    .await
    .result
    .unwrap();
    assert!(
        matches!(result, Output::Audit(ref rows) if rows.iter().any(|row| row.context.request_id==foreign.request_id))
    );
}

#[sqlx::test(migrations = "../db/migrations")]
async fn security_is_admin_only_and_denials_share_request_id(pool: PgPool) {
    let admin = owner(&pool).await;
    let reader = user_login(&pool, Role::Reader, 2).await;
    let denied = service::execute(
        &pool,
        Proof::Session(&reader.session),
        Surface::Console,
        Command::Security(Page::default().into()),
    )
    .await;
    assert!(matches!(denied.result, Err(Error::Forbidden)));
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Security(Page::default().into()),
    )
    .await
    .result
    .unwrap();
    assert!(
        matches!(result, Output::Security(ref rows) if rows.iter().any(|row| row.context.request_id==denied.request_id && row.context.actor_id==Some(reader.account)))
    );
    let invocation: (String, String) = sqlx::query_as(
        "SELECT outcome,error_code FROM management.command_invocations WHERE request_id=$1",
    )
    .bind(denied.request_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(invocation, ("denied".into(), "forbidden".into()));
}

#[sqlx::test(migrations = "../db/migrations")]
async fn secret_free_lists_and_page_bounds(pool: PgPool) {
    let admin = owner(&pool).await;
    assert!(Page::<i64>::new(None, 0).is_err());
    assert!(Page::<i64>::new(None, 101).is_err());
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Credentials {
            account: admin.account,
            page: Page::default(),
        },
    )
    .await
    .result
    .unwrap();
    assert!(!format!("{result:?}").contains(&admin.token));
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::OwnSessions(Page::default()),
    )
    .await
    .result
    .unwrap();
    assert!(!format!("{result:?}").contains(&admin.session));
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Accounts(Page::new(None, 1).unwrap().into()),
    )
    .await
    .result
    .unwrap();
    assert!(matches!(result, Output::Accounts(ref page) if page.items.len()==1));
}
