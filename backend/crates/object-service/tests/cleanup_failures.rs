mod support;

use grove_object_service::cleanup::{CleanupError, cleanup_then_finalize};
use support::Storage;

#[tokio::test]
async fn lost_delete_response_preserves_recovery_until_retry() {
    let storage = Storage::new();
    let result = cleanup_then_finalize(
        || async {
            storage.cleanup().await?;
            Err("delete response lost")
        },
        || storage.finalize(),
    )
    .await;
    assert_eq!(result, Err(CleanupError::Physical("delete response lost")));
    assert_eq!(*storage.calls.borrow(), ["physical"]);
    assert!(storage.recovery.get());
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(true)
    );
}

#[tokio::test]
async fn physical_failure_does_not_even_construct_metadata_operation() {
    let storage = Storage::new();
    let constructed = std::cell::Cell::new(false);
    let result = cleanup_then_finalize(
        || async { Err("delete failed") },
        || {
            constructed.set(true);
            storage.finalize()
        },
    )
    .await;
    assert_eq!(result, Err(CleanupError::Physical("delete failed")));
    assert!(!constructed.get());
    assert!(storage.physical.get());
    assert!(storage.recovery.get());
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(true)
    );
}

#[tokio::test]
async fn metadata_failure_retains_recovery_for_idempotent_retry() {
    let storage = Storage::new();
    let result = cleanup_then_finalize(
        || storage.cleanup(),
        || async { Err::<bool, _>("database unavailable") },
    )
    .await;
    assert_eq!(result, Err(CleanupError::Metadata("database unavailable")));
    assert!(!storage.physical.get());
    assert!(storage.recovery.get());
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(true)
    );
}

#[tokio::test]
async fn lost_commit_response_can_be_retried_without_releasing_twice() {
    let storage = Storage::new();
    let result = cleanup_then_finalize(
        || storage.cleanup(),
        || async {
            storage.finalize().await?;
            Err::<bool, _>("commit response lost")
        },
    )
    .await;
    assert_eq!(result, Err(CleanupError::Metadata("commit response lost")));
    assert!(!storage.recovery.get());
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(false)
    );
}

#[tokio::test]
async fn cancellation_after_physical_cleanup_leaves_recovery_for_next_attempt() {
    use std::future::{Future, pending};
    use std::task::{Context, Waker};

    let storage = Storage::new();
    {
        let mut attempt = std::pin::pin!(cleanup_then_finalize(
            || storage.cleanup(),
            pending::<Result<bool, &'static str>>,
        ));
        let mut context = Context::from_waker(Waker::noop());
        assert!(attempt.as_mut().poll(&mut context).is_pending());
        assert!(!storage.physical.get());
        assert!(storage.recovery.get());
    }
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(true)
    );
}
