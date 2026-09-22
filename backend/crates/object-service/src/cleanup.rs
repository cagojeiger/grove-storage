use std::future::Future;

#[derive(Debug, PartialEq, Eq)]
pub enum CleanupError<Physical, Metadata> {
    Physical(Physical),
    Metadata(Metadata),
}

/// Release durable recovery records only after idempotent physical cleanup succeeds.
///
/// The metadata operation owns atomic fencing and returns false if no transition
/// was applied. Failed or cancelled attempts are retried by the caller's scan;
/// physical cleanup must tolerate already-absent objects. This is not a transaction
/// across storage and the database, and makes no rollback attempt.
pub async fn cleanup_then_finalize<P, M, PF, MF, PE, ME>(
    cleanup: P,
    finalize: M,
) -> Result<bool, CleanupError<PE, ME>>
where
    P: FnOnce() -> PF,
    M: FnOnce() -> MF,
    PF: Future<Output = Result<(), PE>>,
    MF: Future<Output = Result<bool, ME>>,
{
    cleanup().await.map_err(CleanupError::Physical)?;
    finalize().await.map_err(CleanupError::Metadata)
}
