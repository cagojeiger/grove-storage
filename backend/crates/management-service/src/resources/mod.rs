//! Shared resource execution; storage writes await backend probe integration.
mod reads;
mod writes;

use crate::{Error, Proof, audit_context, logging};
use filegate_core::Crypto;
use filegate_db::{
    PgPool,
    management::{AuditContext, IdentityTransaction},
};
use grove_management_command::{Command, CommandError, Effect, ErrorCode, Outcome, Output};
use grove_management_policy::{Scope, Surface, authorize};
use uuid::Uuid;

pub struct Execution {
    pub request_id: Uuid,
    pub result: Result<Output, CommandError>,
}

pub async fn execute(
    pool: &PgPool,
    crypto: &Crypto,
    proof: Proof<'_>,
    surface: Surface,
    command: Command,
) -> Execution {
    let request_id = Uuid::new_v4();
    let started = std::time::Instant::now();
    let name = command.name();
    let (context, result) = run(pool, crypto, proof, surface, command, request_id).await;
    logging::record(
        pool,
        context.as_ref(),
        request_id,
        surface,
        name.as_str(),
        started.elapsed(),
        &result,
    )
    .await;
    Execution {
        request_id,
        result: result.map_err(wire_error),
    }
}

async fn run(
    pool: &PgPool,
    crypto: &Crypto,
    proof: Proof<'_>,
    surface: Surface,
    command: Command,
    request_id: Uuid,
) -> (Option<AuditContext>, Result<Output, Error>) {
    let mut tx = match IdentityTransaction::begin(pool).await {
        Ok(tx) => tx,
        Err(error) => return (None, Err(error.into())),
    };
    let identity = match tx.resolve(proof).await {
        Ok(Some(identity)) => identity,
        Ok(None) => return (None, Err(Error::Unauthenticated)),
        Err(error) => return (None, Err(error.into())),
    };
    let context = match audit_context(&identity, request_id, surface) {
        Ok(context) => context,
        Err(error) => return (None, Err(error)),
    };
    if authorize(identity.caller, surface, command.name().required_action())
        != Ok(Scope::Installation)
    {
        return (Some(context), Err(Error::Forbidden));
    }
    if command.validate().is_err() {
        return (Some(context), Err(Error::InvalidInput));
    }
    let mutation = command.name().effect() == Effect::Mutation;
    let result = if mutation {
        writes::run(&mut tx, crypto, &context, command).await
    } else {
        reads::run(&mut tx, command).await
    };
    let result = match result {
        Ok(output) => tx.finish().await.map(|()| output).map_err(|_| {
            if mutation {
                Error::OutcomeUnknown
            } else {
                Error::Unavailable
            }
        }),
        Err(error) => Err(error),
    };
    (Some(context), result)
}

fn wire_error(error: Error) -> CommandError {
    if error == Error::OutcomeUnknown {
        return CommandError {
            code: ErrorCode::Unavailable,
            outcome: Outcome::Unknown,
        };
    }
    CommandError::rejected(match error {
        Error::Unauthenticated => ErrorCode::Unauthorized,
        Error::Forbidden => ErrorCode::Forbidden,
        Error::NotFound => ErrorCode::NotFound,
        Error::Conflict => ErrorCode::Conflict,
        Error::InvalidInput => ErrorCode::InvalidInput,
        Error::RequestRejected => ErrorCode::RequestRejected,
        Error::Unavailable | Error::OutcomeUnknown | Error::RateLimited => ErrorCode::Unavailable,
    })
}
