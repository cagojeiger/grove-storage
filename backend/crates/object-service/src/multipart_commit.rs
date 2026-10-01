//! Native multipart completion after ownership and write-lease lookup.
use std::future::Future;

pub trait MultipartCommit {
    type Error;
    type Prepared;

    /// Validate a stable part snapshot and durably claim completion. None means
    /// another transition won; physical completion must not run in that case.
    fn prepare(&self) -> impl Future<Output = Result<Option<Self::Prepared>, Self::Error>> + Send;
    /// Complete exactly the prepared snapshot under a renewable ownership fence.
    /// None means ownership was lost; this attempt must not finalize metadata.
    fn complete(
        &self,
        prepared: &Self::Prepared,
    ) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send;
    fn finalize(&self, etag: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommitError<E> {
    Operation(E),
    OwnershipLost,
}

/// None asks the caller to reload the current state for its idempotent response.
/// Errors and cancellation may follow an already-applied effect. This path never
/// deletes or reopens uncertain output; durable state governs reconciliation.
pub async fn commit<O: MultipartCommit>(
    operations: &O,
) -> Result<Option<String>, CommitError<O::Error>> {
    let Some(prepared) = operations.prepare().await.map_err(CommitError::Operation)? else {
        return Ok(None);
    };
    let etag = operations
        .complete(&prepared)
        .await
        .map_err(CommitError::Operation)?
        .ok_or(CommitError::OwnershipLost)?;
    let finalized = operations
        .finalize(&etag)
        .await
        .map_err(CommitError::Operation)?;
    Ok(finalized.then_some(etag))
}
