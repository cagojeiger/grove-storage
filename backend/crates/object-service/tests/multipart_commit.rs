#![allow(clippy::unwrap_used)]
#[path = "support/multipart_commit.rs"]
mod support;
use grove_object_service::multipart_commit::commit;
use support::Fake;

#[tokio::test]
async fn success_uses_the_claimed_snapshot_and_verified_etag_in_order() {
    let fake = Fake::new();
    assert_eq!(commit(&fake).await, Ok(Some("verified-etag".into())));
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
async fn lost_claim_requests_current_state_without_physical_completion() {
    let mut fake = Fake::new();
    fake.prepared = Ok(None);
    assert_eq!(commit(&fake).await, Ok(None));
    assert_eq!(*fake.calls.lock().unwrap(), ["prepare"]);
}

#[tokio::test]
async fn lost_final_transition_requests_current_state_instead_of_reporting_success() {
    let mut fake = Fake::new();
    fake.finalized = Ok(false);
    assert_eq!(commit(&fake).await, Ok(None));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        [
            "prepare",
            "complete:claimed-snapshot",
            "finalize:verified-etag",
        ]
    );
}
