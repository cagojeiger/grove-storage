#![allow(clippy::unwrap_used)]
#[path = "support/single_commit.rs"]
mod support;
use grove_object_service::single_commit::{CommitError, SingleCommit, commit};
use support::Fake;

#[tokio::test]
async fn observation_failure_is_not_absence_and_never_finalizes() {
    let mut fake = Fake::new();
    fake.observation = Err("provider unavailable");
    assert_eq!(
        commit(&fake, 7, None).await,
        Err(CommitError::Operation("provider unavailable"))
    );
    assert_eq!(*fake.calls.lock().unwrap(), ["observe"]);
}

#[tokio::test]
async fn finalization_error_propagates_without_guessing_or_retrying_the_write() {
    let mut fake = Fake::new();
    fake.finalized = Err("commit response lost");
    assert_eq!(
        commit(&fake, 7, None).await,
        Err(CommitError::Operation("commit response lost"))
    );
    assert_eq!(*fake.calls.lock().unwrap(), ["observe", "finalize:etag"]);
}

#[tokio::test]
async fn reload_errors_preserve_the_adapters_ownership_or_database_failure() {
    for error in ["not found", "database unavailable"] {
        let mut fake = Fake::new();
        fake.finalized = Ok(false);
        fake.current = Err(error);
        assert_eq!(
            commit(&fake, 7, None).await,
            Err(CommitError::Operation(error))
        );
        assert_eq!(
            *fake.calls.lock().unwrap(),
            ["observe", "finalize:etag", "reload"]
        );
    }
}

#[tokio::test]
async fn cancellation_during_finalization_does_not_start_a_reload() {
    use std::future::{Future, pending};
    use std::task::{Context, Waker};

    struct Paused(Fake);
    impl SingleCommit for Paused {
        type Error = &'static str;
        async fn observe(&self) -> Result<Option<(i64, String)>, Self::Error> {
            self.0.observe().await
        }
        async fn finalize(&self, etag: &str) -> Result<bool, Self::Error> {
            self.0.record(format!("finalize:{etag}"));
            pending().await
        }
        async fn committed_etag(&self) -> Result<Option<String>, Self::Error> {
            self.0.committed_etag().await
        }
    }
    let fake = Paused(Fake::new());
    {
        let mut attempt = std::pin::pin!(commit(&fake, 7, None));
        let mut context = Context::from_waker(Waker::noop());
        assert!(attempt.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(*fake.0.calls.lock().unwrap(), ["observe", "finalize:etag"]);
}
