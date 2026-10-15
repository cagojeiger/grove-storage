use crate::{Error, Proof};
use grove_db::{
    PgPool,
    management::{
        self as db, AuditActor, AuditContext,
        telemetry::{self, Outcome, SecurityReason},
    },
};
use grove_management_policy::Surface;
use std::time::Duration;
use uuid::Uuid;

pub(crate) use telemetry::{AuthMethod, Details, Stage, Target};

const LOG_TIMEOUT: Duration = Duration::from_millis(250);

pub(crate) fn details(operation: &'static str, proof: Proof<'_>) -> Details {
    let mut details = Details::new(operation, Stage::Authentication);
    details.auth_method = Some(match proof {
        Proof::Session(_) | Proof::PasswordSession(_) => AuthMethod::Session,
        Proof::Token(_) => AuthMethod::ApiToken,
    });
    details
}

pub(crate) async fn session_operation<T>(
    pool: &PgPool,
    request_id: Uuid,
    session_hash: &str,
    mut details: Details,
    work: impl AsyncFnOnce(&mut Details) -> Result<T, Error>,
) -> Result<T, Error> {
    let started = tokio::time::Instant::now();
    let context = db::session_actor(pool, session_hash)
        .await
        .ok()
        .flatten()
        .map(|actor| AuditContext {
            actor: AuditActor::Account {
                id: actor.account_id,
                credential_id: actor.credential_id,
                session_id: actor.session_id,
            },
            request_id,
            surface: Surface::Console,
        });
    details.auth_method = Some(AuthMethod::Session);
    if let Some(context) = &context {
        details.stage = Stage::Execution;
        if details.target.is_none()
            && details.operation != "identity.account.create"
            && let AuditActor::Account { id, .. } = context.actor
        {
            details.target = Some(Target::account(id));
        }
    }
    let result = work(&mut details).await;
    if matches!(result, Err(Error::OutcomeUnknown)) {
        details.stage = Stage::Commit;
    }
    record(
        pool,
        context.as_ref(),
        request_id,
        Surface::Console,
        &details,
        started.elapsed(),
        &result,
    )
    .await;
    result
}

pub(super) async fn record<T>(
    pool: &PgPool,
    context: Option<&AuditContext>,
    request_id: Uuid,
    surface: Surface,
    details: &Details,
    elapsed: Duration,
    result: &Result<T, Error>,
) {
    observe(request_id, surface, details.operation, elapsed, result);
    if let Some(context) = context.filter(|_| result.is_err() || records_success(details.operation))
    {
        let (outcome, code) = match result {
            Ok(_) => (Outcome::Succeeded, None),
            Err(Error::Forbidden) => (Outcome::Denied, Some(Error::Forbidden.code())),
            Err(Error::OutcomeUnknown) => (Outcome::Unknown, Some(Error::OutcomeUnknown.code())),
            Err(error) => (Outcome::Failed, Some(error.code())),
        };
        let write = telemetry::invocation(
            pool,
            context,
            details,
            outcome,
            code,
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        );
        if !matches!(tokio::time::timeout(LOG_TIMEOUT, write).await, Ok(Ok(()))) {
            tracing::error!(event="history.write_failed", operation_id=%request_id,
                stream="command_invocations", error_code="write_failed_or_timed_out", outcome="error");
        }
    }
    let reason = match result {
        Err(Error::Unauthenticated) => Some(SecurityReason::Unauthenticated),
        Err(Error::Forbidden) => Some(SecurityReason::Forbidden),
        Err(Error::RateLimited) => Some(SecurityReason::RateLimited),
        Err(Error::Unavailable)
            if matches!(
                details.stage,
                Stage::Authentication | Stage::Reauthentication
            ) =>
        {
            Some(SecurityReason::Unavailable)
        }
        _ => None,
    };
    if let Some(reason) = reason {
        security(pool, context, request_id, surface, reason, details).await;
    }
}

pub(super) async fn security(
    pool: &PgPool,
    context: Option<&AuditContext>,
    request_id: Uuid,
    surface: Surface,
    reason: SecurityReason,
    details: &Details,
) {
    let write = telemetry::security(pool, context, request_id, surface, reason, details);
    if !matches!(tokio::time::timeout(LOG_TIMEOUT, write).await, Ok(Ok(()))) {
        tracing::error!(event="history.write_failed", operation_id=%request_id,
            stream="security_events", error_code="write_failed_or_timed_out", outcome="error");
    }
}

pub(crate) async fn reauthenticated(
    pool: &PgPool,
    request_id: Uuid,
    actor: &db::Identity,
    details: &Details,
) {
    let context = AuditContext {
        actor: AuditActor::Account {
            id: actor.account_id,
            credential_id: actor.credential_id,
            session_id: actor.session_id,
        },
        request_id,
        surface: Surface::Console,
    };
    security(
        pool,
        Some(&context),
        request_id,
        Surface::Console,
        SecurityReason::Authenticated,
        details,
    )
    .await;
}

fn records_success(operation: &str) -> bool {
    use grove_management_command::{CommandName, Effect};
    if let Some(name) = CommandName::parse(operation) {
        return name.effect() == Effect::Mutation
            || matches!(
                name,
                CommandName::StorageTest | CommandName::CredentialList | CommandName::ClientKeyList
            );
    }
    matches!(
        operation,
        "identity.account.create"
            | "identity.account.name"
            | "identity.account.role"
            | "identity.account.active"
            | "identity.account.delete"
            | "identity.account.password_setup.issue"
            | "identity.credential.issue"
            | "identity.credential.revoke"
            | "identity.credential.list"
            | "identity.session.login"
            | "identity.session.logout"
            | "identity.session.revoke"
            | "account.password_change"
    )
}

fn observe<T>(
    id: Uuid,
    surface: Surface,
    operation: &'static str,
    elapsed: Duration,
    result: &Result<T, Error>,
) {
    let surface = match surface {
        Surface::Console => "console",
        Surface::Cli => "cli",
        Surface::Mcp => "mcp",
        Surface::ResourceApi => "resource_api",
    };
    let duration_ms = elapsed.as_millis() as u64;
    let outcome = match result {
        Ok(_) => "success",
        Err(Error::OutcomeUnknown) => "unknown",
        Err(
            Error::Forbidden
            | Error::Unauthenticated
            | Error::RateLimited
            | Error::InvalidInput
            | Error::NotFound
            | Error::Conflict
            | Error::RequestRejected,
        ) => "rejected",
        Err(_) => "error",
    };
    let error_code = result
        .as_ref()
        .err()
        .map(|error| error.code())
        .unwrap_or("");
    if matches!(outcome, "error" | "unknown") {
        tracing::error!(event="management.operation_finished", operation_id=%id,
            surface, operation, outcome, error_code, duration_ms);
    } else if outcome == "rejected" {
        tracing::warn!(event="management.operation_finished", operation_id=%id,
            surface, operation, outcome, error_code, duration_ms);
    } else if is_read(operation) {
        tracing::debug!(event="management.operation_finished", operation_id=%id,
            surface, operation, outcome, duration_ms);
    } else {
        tracing::info!(event="management.operation_finished", operation_id=%id,
            surface, operation, outcome, duration_ms);
    }
}

fn is_read(operation: &str) -> bool {
    use grove_management_command::{CommandName, Effect};
    CommandName::parse(operation)
        .map(|name| name.effect() == Effect::Read)
        .unwrap_or_else(|| {
            operation.ends_with(".list")
                || operation.ends_with(".get")
                || operation == "identity.session.current"
        })
}

#[cfg(test)]
mod tests {
    use super::{is_read, records_success};
    use grove_management_command::{CommandName, Effect};

    #[test]
    fn resource_log_levels_follow_catalog_effects() {
        for name in CommandName::ALL {
            assert_eq!(is_read(name.as_str()), name.effect() == Effect::Read);
        }
        assert!(is_read("identity.session.current"));
        assert!(!is_read("identity.account.create"));
    }

    #[test]
    fn successful_history_uses_an_explicit_read_allowlist() {
        for name in CommandName::ALL {
            assert_eq!(
                records_success(name.as_str()),
                name.effect() == Effect::Mutation
                    || matches!(
                        name,
                        CommandName::StorageTest
                            | CommandName::CredentialList
                            | CommandName::ClientKeyList
                    )
            );
        }
        for name in [
            "identity.account.list",
            "identity.account.get",
            "identity.session.current",
            "identity.session.list",
            "history.audit.list",
            "history.invocation.list",
            "history.security.list",
            "unknown.command",
        ] {
            assert!(!records_success(name));
        }
        assert!(records_success("identity.credential.list"));
        assert!(records_success("identity.account.create"));
    }

    #[sqlx::test(migrations = "../db/migrations")]
    #[allow(clippy::unwrap_used)]
    async fn invocation_pool_wait_is_bounded_without_changing_the_operation_result(
        options: sqlx::postgres::PgPoolOptions,
        connect: sqlx::postgres::PgConnectOptions,
    ) {
        use super::*;
        let pool = options
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(60))
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(connect)
            .await
            .unwrap();
        let mut held = pool.acquire().await.unwrap();
        let context = AuditContext {
            actor: AuditActor::System,
            request_id: Uuid::new_v4(),
            surface: Surface::Console,
        };
        let result: Result<(), Error> = Ok(());
        let details = Details::new("account.password_change", Stage::Execution);
        tokio::time::pause();
        let started = tokio::time::Instant::now();
        let mut write = Box::pin(record(
            &pool,
            Some(&context),
            context.request_id,
            Surface::Console,
            &details,
            Duration::ZERO,
            &result,
        ));
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(write.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        tokio::time::advance(LOG_TIMEOUT - Duration::from_millis(1)).await;
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(write.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        tokio::time::advance(Duration::from_millis(1)).await;
        write.await;
        assert!(started.elapsed() >= LOG_TIMEOUT);
        assert!(started.elapsed() <= LOG_TIMEOUT + Duration::from_millis(1));
        tokio::time::resume();
        assert!(result.is_ok());
        let rows: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM management.command_invocations WHERE request_id=$1",
        )
        .bind(context.request_id)
        .fetch_one(&mut *held)
        .await
        .unwrap();
        assert_eq!(rows, 0);
        drop(held);
        pool.close().await;
    }
}
