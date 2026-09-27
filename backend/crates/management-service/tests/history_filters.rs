#![allow(clippy::unwrap_used, clippy::unreachable)]
mod support;
use filegate_db::{
    PgPool,
    management::{self as db, AuditActor, AuditContext, history::HistoryQuery, telemetry},
};
use grove_management_policy::{Role, Surface};
use grove_management_service::{self as service, Command, Output, Page, Proof};
use support::*;
use uuid::Uuid;

fn query(account: Option<Uuid>, credential: Option<Uuid>, before: Option<i64>) -> HistoryQuery {
    HistoryQuery {
        account_id: account,
        credential_id: credential,
        page: Page::new(before, 1).unwrap(),
    }
}

#[sqlx::test(migrations = "../db/migrations")]
async fn actor_and_token_filters_intersect_scope_before_pagination(pool: PgPool) {
    let admin = owner(&pool).await;
    let reader = user_login(&pool, Role::Reader, 2).await;
    let writer = user_login(&pool, Role::Writer, 3).await;
    let second = db::issue_credential(&pool, &context(), reader.account, &key(&hash(4)))
        .await
        .unwrap();
    let mut selected = Vec::new();
    for (id, credential) in [
        (reader.account, reader.credential),
        (reader.account, reader.credential),
        (reader.account, second.id),
        (writer.account, writer.credential),
    ] {
        let ctx = AuditContext {
            actor: AuditActor::User {
                id,
                credential_id: credential,
                session_id: None,
            },
            request_id: Uuid::new_v4(),
            surface: Surface::Mcp,
        };
        // Seed immutable events independently of transport authorization.
        db::create_account(
            &pool,
            &ctx,
            db::NewAccount {
                display_name: "target",
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
        telemetry::security(
            &pool,
            Some(&ctx),
            ctx.request_id,
            Surface::Mcp,
            telemetry::SecurityReason::Forbidden,
        )
        .await
        .unwrap();
        if credential == reader.credential {
            selected.push(ctx.request_id);
        }
    }
    for stream in ["audit", "invocations", "security"] {
        let mut before = None;
        for expected in selected.iter().rev() {
            let filter = query(Some(reader.account), Some(reader.credential), before);
            let command = match stream {
                "audit" => Command::Audit(filter),
                "invocations" => Command::Invocations(filter),
                _ => Command::Security(filter),
            };
            let result = service::execute(
                &pool,
                Proof::Session(&admin.session),
                Surface::Console,
                command,
            )
            .await
            .result
            .unwrap();
            let contexts: Vec<_> = match result {
                Output::Audit(rows) => rows.into_iter().map(|r| r.context).collect(),
                Output::Invocations(rows) => rows.into_iter().map(|r| r.context).collect(),
                Output::Security(rows) => rows.into_iter().map(|r| r.context).collect(),
                _ => unreachable!(),
            };
            assert_eq!(contexts.len(), 1);
            let event = contexts.first().unwrap();
            assert_eq!(event.request_id, *expected);
            before = Some(event.id);
        }
    }
    for viewer in [&reader, &writer] {
        let foreign = if viewer.account == reader.account {
            &writer
        } else {
            &reader
        };
        for filter in [
            query(Some(foreign.account), None, None),
            query(None, Some(foreign.credential), None),
        ] {
            let result = service::execute(
                &pool,
                Proof::Session(&viewer.session),
                Surface::Console,
                Command::Audit(filter),
            )
            .await
            .result
            .unwrap();
            assert!(matches!(result, Output::Audit(rows) if rows.is_empty()));
        }
        let result = service::execute(
            &pool,
            Proof::Session(&viewer.session),
            Surface::Console,
            Command::Invocations(query(None, Some(foreign.credential), None)),
        )
        .await
        .result
        .unwrap();
        assert!(matches!(result, Output::Invocations(rows) if rows.is_empty()));
    }
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Audit(query(Some(writer.account), Some(reader.credential), None)),
    )
    .await
    .result
    .unwrap();
    assert!(matches!(result, Output::Audit(rows) if rows.is_empty()));
    db::revoke_credential(&pool, &context(), reader.credential)
        .await
        .unwrap();
    db::change_account(&pool, &context(), reader.account, db::AccountChange::Delete)
        .await
        .unwrap();
    let result = service::execute(
        &pool,
        Proof::Session(&admin.session),
        Surface::Console,
        Command::Audit(query(Some(reader.account), Some(reader.credential), None)),
    )
    .await
    .result
    .unwrap();
    assert!(
        matches!(result, Output::Audit(rows) if rows.first().unwrap().context.request_id == *selected.last().unwrap())
    );
}
