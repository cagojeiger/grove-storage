#![allow(clippy::unwrap_used)]
#[path = "support/multipart_commit.rs"]
mod support;
use grove_object_service::multipart_commit::{CommitError, MultipartCommit, commit};
use support::Fake;

#[tokio::test]
async fn validation_or_claim_failure_never_starts_physical_completion() {
    for error in ["missing parts", "part upload busy", "claim failed"] {
        let mut fake = Fake::new();
        fake.prepared = Err(error);
        assert_eq!(commit(&fake).await, Err(CommitError::Operation(error)));
        assert_eq!(*fake.calls.lock().unwrap(), ["prepare"]);
    }
}

#[tokio::test]
async fn physical_failure_or_invalid_etag_never_finalizes_metadata() {
    for error in ["complete response lost", "etag mismatch"] {
        let mut fake = Fake::new();
        fake.completed = Err(error);
        assert_eq!(commit(&fake).await, Err(CommitError::Operation(error)));
        assert_eq!(
            *fake.calls.lock().unwrap(),
            ["prepare", "complete:claimed-snapshot"]
        );
    }
}

#[tokio::test]
async fn lost_heartbeat_ownership_never_finalizes_metadata() {
    let mut fake = Fake::new();
    fake.completed = Ok(None);
    assert_eq!(commit(&fake).await, Err(CommitError::OwnershipLost));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        ["prepare", "complete:claimed-snapshot"]
    );
}

#[tokio::test]
async fn finalization_error_does_not_repeat_physical_completion() {
    let mut fake = Fake::new();
    fake.finalized = Err("database response lost");
    assert_eq!(
        commit(&fake).await,
        Err(CommitError::Operation("database response lost"))
    );
    assert_eq!(
        *fake.calls.lock().unwrap(),
        [
            "prepare",
            "complete:claimed-snapshot",
            "finalize:verified-etag",
        ]
    );
}

#[tokio::test]
async fn cancellation_at_each_stage_does_not_start_the_next_operation() {
    use std::future::{Future, pending};
    use std::task::{Context, Waker};

    struct Paused {
        fake: Fake,
        stage: u8,
    }
    impl MultipartCommit for Paused {
        type Error = &'static str;
        type Prepared = &'static str;
        async fn prepare(&self) -> Result<Option<Self::Prepared>, Self::Error> {
            let result = self.fake.prepare().await;
            if self.stage == 0 {
                pending::<()>().await;
            }
            result
        }
        async fn complete(&self, prepared: &Self::Prepared) -> Result<Option<String>, Self::Error> {
            let result = self.fake.complete(prepared).await;
            if self.stage == 1 {
                pending::<()>().await;
            }
            result
        }
        async fn finalize(&self, etag: &str) -> Result<bool, Self::Error> {
            let result = self.fake.finalize(etag).await;
            if self.stage == 2 {
                pending::<()>().await;
            }
            result
        }
    }
    for stage in 0..3 {
        let operations = Paused {
            fake: Fake::new(),
            stage,
        };
        {
            let mut attempt = std::pin::pin!(commit(&operations));
            let mut context = Context::from_waker(Waker::noop());
            assert!(attempt.as_mut().poll(&mut context).is_pending());
        }
        assert_eq!(
            operations.fake.calls.lock().unwrap().len(),
            usize::from(stage) + 1
        );
    }
}
