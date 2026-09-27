#![allow(clippy::unwrap_used)]
#[path = "support/single_commit.rs"]
mod support;
use grove_object_service::single_commit::{CommitError, Committed, commit};
use support::Fake;

#[tokio::test]
async fn successful_commit_observes_before_finalizing_without_a_reload() {
    for size in [0, 7] {
        let mut fake = Fake::new();
        fake.observation = Ok(Some((size, "etag".into())));
        assert_eq!(
            commit(&fake, size, None).await,
            Ok(Committed {
                etag: "etag".into(),
                transitioned: true,
            })
        );
        assert_eq!(*fake.calls.lock().unwrap(), ["observe", "finalize:etag"]);
    }
}

#[tokio::test]
async fn md5_comparison_ignores_case_but_preserves_the_observed_etag() {
    let mut fake = Fake::new();
    fake.observation = Ok(Some((7, "ABCDEF".into())));
    assert_eq!(
        commit(&fake, 7, Some("abcdef")).await,
        Ok(Committed {
            etag: "ABCDEF".into(),
            transitioned: true,
        })
    );
    assert_eq!(*fake.calls.lock().unwrap(), ["observe", "finalize:ABCDEF"]);
}

#[tokio::test]
async fn missing_upload_does_not_write_metadata() {
    let mut fake = Fake::new();
    fake.observation = Ok(None);
    assert_eq!(
        commit(&fake, 7, None).await,
        Err(CommitError::MissingUpload)
    );
    assert_eq!(*fake.calls.lock().unwrap(), ["observe"]);
}

#[tokio::test]
async fn size_mismatch_precedes_checksum_mismatch_and_never_finalizes() {
    let fake = Fake::new();
    assert_eq!(
        commit(&fake, 8, Some("different")).await,
        Err(CommitError::SizeMismatch)
    );
    assert_eq!(*fake.calls.lock().unwrap(), ["observe"]);
}

#[tokio::test]
async fn checksum_mismatch_does_not_write_metadata() {
    let fake = Fake::new();
    assert_eq!(
        commit(&fake, 7, Some("different")).await,
        Err(CommitError::Md5Mismatch)
    );
    assert_eq!(*fake.calls.lock().unwrap(), ["observe"]);
}

#[tokio::test]
async fn losing_a_transition_returns_the_winners_etag_not_the_observation() {
    let mut fake = Fake::new();
    fake.finalized = Ok(false);
    assert_eq!(
        commit(&fake, 7, None).await,
        Ok(Committed {
            etag: "winner".into(),
            transitioned: false,
        })
    );
    assert_eq!(
        *fake.calls.lock().unwrap(),
        ["observe", "finalize:etag", "reload"]
    );
}

#[tokio::test]
async fn losing_to_a_non_active_state_is_not_a_success() {
    let mut fake = Fake::new();
    fake.finalized = Ok(false);
    fake.current = Ok(None);
    assert_eq!(
        commit(&fake, 7, None).await,
        Err(CommitError::NotCommittable)
    );
    assert_eq!(
        *fake.calls.lock().unwrap(),
        ["observe", "finalize:etag", "reload"]
    );
}
