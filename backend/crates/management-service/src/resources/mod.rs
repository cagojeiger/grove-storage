//! Shared resource execution; external probes run outside the identity fence.
mod admission;
mod credentials;
mod output;
mod reads;
mod storage;
mod writes;

use crate::{Error, Proof, audit_context, logging};
use grove_core::Crypto;
use grove_db::{
    PgPool,
    management::{AuditContext, IdentityTransaction},
    registry::StorageRow,
};
use grove_management_command::{
    Command, CommandError, Effect, ErrorCode, Outcome, Output, input::StorageInput,
};
use grove_management_policy::{Scope, Surface, authorize};
use std::future::Future;
use uuid::Uuid;

pub use admission::admit_mcp;
pub use credentials::PreparedCredential;
pub use output::{client_usage_output, snapshot_output, storage_output, storage_usage_output};

pub struct Execution {
    pub request_id: Uuid,
    pub result: Result<Output, CommandError>,
}

pub enum StorageOperation {
    Register(StorageInput),
    Test(StorageRow),
}

/// The trusted verifier registers submitted settings or probes a saved row.
/// Tests return the unchanged row. All network work runs outside the transaction.
pub async fn execute<V, F>(
    pool: &PgPool,
    crypto: &Crypto,
    verify_storage: V,
    proof: Proof<'_>,
    surface: Surface,
    command: Command,
) -> Execution
where
    V: FnOnce(StorageOperation) -> F,
    F: Future<Output = Result<StorageRow, Error>>,
{
    let request_id = Uuid::new_v4();
    let started = tokio::time::Instant::now();
    let name = command.name();
    let (context, result) = run(
        pool,
        crypto,
        verify_storage,
        proof,
        surface,
        command,
        request_id,
    )
    .await;
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

async fn run<V, F>(
    pool: &PgPool,
    crypto: &Crypto,
    verify_storage: V,
    proof: Proof<'_>,
    surface: Surface,
    command: Command,
    request_id: Uuid,
) -> (Option<AuditContext>, Result<Output, Error>)
where
    V: FnOnce(StorageOperation) -> F,
    F: Future<Output = Result<StorageRow, Error>>,
{
    let mut context = None;
    let name = command.name();
    let mutation = command.name().effect() == Effect::Mutation;
    let result = async {
        let mut tx = IdentityTransaction::begin(pool).await?;
        let mut ctx = current(&mut tx, proof, surface, name, request_id, &mut context).await?;
        command.validate().map_err(|_| Error::InvalidInput)?;
        let output = match command {
            Command::StorageCreate(input) | Command::StorageReplace(input) => {
                if name == grove_management_command::CommandName::StorageReplace {
                    tx.storage(&input.id).await?;
                }
                // No resource change yet. Release both the DB connection and identity lock.
                tx.finish().await.map_err(|_| Error::Unavailable)?;
                let row = verify_storage(StorageOperation::Register(input)).await?;
                tx = IdentityTransaction::begin(pool).await?;
                ctx = current(&mut tx, proof, surface, name, request_id, &mut context).await?;
                storage::write(&mut tx, &ctx, name, row).await?
            }
            Command::StorageTest(input) => {
                let row = tx.storage(&input.id).await?;
                if row.kind != "s3" {
                    return Err(Error::InvalidInput);
                }
                tx.finish().await.map_err(|_| Error::Unavailable)?;
                let probe = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    verify_storage(StorageOperation::Test(row.clone())),
                )
                .await;
                tx = IdentityTransaction::begin(pool).await?;
                current(&mut tx, proof, surface, name, request_id, &mut context).await?;
                if tx.storage(&input.id).await? != row {
                    return Err(Error::Conflict);
                }
                probe.map_err(|_| Error::Unavailable)??;
                Output::StorageTest(grove_management_command::model::StorageConnection {
                    id: input.id,
                    state: grove_management_command::model::State::Ok,
                })
            }
            command if mutation => writes::run(&mut tx, crypto, &ctx, command).await?,
            command => reads::run(&mut tx, command).await?,
        };
        tx.finish().await.map(|()| output).map_err(|_| {
            if mutation {
                Error::OutcomeUnknown
            } else {
                Error::Unavailable
            }
        })
    }
    .await;
    (context, result)
}

async fn current(
    tx: &mut IdentityTransaction<'_>,
    proof: Proof<'_>,
    surface: Surface,
    name: grove_management_command::CommandName,
    request_id: Uuid,
    context: &mut Option<AuditContext>,
) -> Result<AuditContext, Error> {
    *context = None;
    let identity = tx.resolve(proof).await?.ok_or(Error::Unauthenticated)?;
    let ctx = audit_context(&identity, request_id, surface)?;
    *context = Some(ctx);
    if authorize(identity.caller(), surface, name.required_action()) != Ok(Scope::Installation) {
        return Err(Error::Forbidden);
    }
    Ok(ctx)
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
