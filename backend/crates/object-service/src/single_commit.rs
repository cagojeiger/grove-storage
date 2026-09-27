//! Native single-upload completion after ownership and multipart routing checks.
use std::future::Future;

pub trait SingleCommit {
    type Error;

    /// Direct uploads use provider metadata; relays use durable upload measurements.
    fn observe(&self) -> impl Future<Output = Result<Option<(i64, String)>, Self::Error>> + Send;
    /// Atomically transition pending to active and settle the write lease.
    fn finalize(&self, etag: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    /// Recheck ownership and return the winning ETag only if the file is active.
    fn committed_etag(&self) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send;
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommitError<E> {
    Operation(E),
    MissingUpload,
    SizeMismatch,
    Md5Mismatch,
    NotCommittable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Committed {
    pub etag: String,
    pub transitioned: bool,
}

/// Observation and validation precede every write. A lost transition reloads the
/// winning state; an operation error propagates without guessing its outcome.
pub async fn commit<O: SingleCommit>(
    operations: &O,
    declared_size: i64,
    declared_md5: Option<&str>,
) -> Result<Committed, CommitError<O::Error>> {
    let (size, etag) = operations
        .observe()
        .await
        .map_err(CommitError::Operation)?
        .ok_or(CommitError::MissingUpload)?;
    if size != declared_size {
        return Err(CommitError::SizeMismatch);
    }
    if declared_md5.is_some_and(|md5| !md5.eq_ignore_ascii_case(&etag)) {
        return Err(CommitError::Md5Mismatch);
    }
    if operations
        .finalize(&etag)
        .await
        .map_err(CommitError::Operation)?
    {
        return Ok(Committed {
            etag,
            transitioned: true,
        });
    }
    let etag = operations
        .committed_etag()
        .await
        .map_err(CommitError::Operation)?
        .ok_or(CommitError::NotCommittable)?;
    Ok(Committed {
        etag,
        transitioned: false,
    })
}
