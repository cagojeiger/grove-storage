mod support;

use grove_object_service::cleanup::cleanup_then_finalize;
use support::Storage;

#[tokio::test]
async fn metadata_fence_rejection_is_not_reported_as_applied() {
    let storage = Storage::new();
    assert_eq!(
        cleanup_then_finalize(
            || storage.cleanup(),
            || async { Ok::<_, &'static str>(false) }
        )
        .await,
        Ok(false)
    );
    assert!(storage.recovery.get());
}

#[tokio::test]
async fn physical_cleanup_precedes_metadata_release() {
    let storage = Storage::new();
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(true)
    );
    assert_eq!(*storage.calls.borrow(), ["physical", "metadata"]);
    assert!(!storage.physical.get());
    assert!(!storage.recovery.get());
}

#[tokio::test]
async fn repeated_cleanup_preserves_no_transition_outcome() {
    let storage = Storage::new();
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(true)
    );
    assert_eq!(
        cleanup_then_finalize(|| storage.cleanup(), || storage.finalize()).await,
        Ok(false)
    );
    assert_eq!(
        *storage.calls.borrow(),
        ["physical", "metadata", "physical", "metadata"]
    );
}
