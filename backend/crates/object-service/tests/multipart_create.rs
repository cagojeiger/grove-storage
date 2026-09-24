#![allow(clippy::unwrap_used)]

use grove_object_service::cleanup::cleanup_then_finalize;
use grove_object_service::multipart_create::{MultipartCreate, initialize};
use std::future::{Future, ready};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Create,
    Attach,
    RelaySecret,
    RelayWrite,
}

struct Fake {
    vendor: bool,
    failure: Option<Failure>,
    abort_fails: bool,
    reclaim_fails: bool,
    recovery: AtomicBool,
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn new(vendor: bool, failure: Option<Failure>) -> Self {
        Self {
            vendor,
            failure,
            abort_fails: false,
            reclaim_fails: false,
            recovery: AtomicBool::new(true),
            calls: Mutex::new(Vec::new()),
        }
    }
    fn record(&self, call: impl Into<String>) {
        self.calls.lock().unwrap().push(call.into());
    }
}

impl MultipartCreate for Fake {
    type Error = Failure;
    fn create_vendor_upload(&self) -> impl Future<Output = Result<Option<String>, Failure>> + Send {
        self.record("create");
        ready(if self.failure == Some(Failure::Create) {
            Err(Failure::Create)
        } else {
            Ok(self.vendor.then(|| "vendor-id".to_owned()))
        })
    }
    fn attach_vendor_upload(&self, id: &str) -> impl Future<Output = Result<(), Failure>> + Send {
        self.record(format!("attach:{id}"));
        ready(if self.failure == Some(Failure::Attach) {
            Err(Failure::Attach)
        } else {
            Ok(())
        })
    }
    fn prepare_relay(&self) -> impl Future<Output = Result<(), Failure>> + Send {
        self.record("relay");
        ready(match self.failure {
            Some(error @ (Failure::RelaySecret | Failure::RelayWrite)) => Err(error),
            _ => Ok(()),
        })
    }
    async fn compensate(&self, id: Option<&str>) {
        let _ = cleanup_then_finalize(
            || async {
                if self.vendor {
                    self.record(format!("abort:{}", id.unwrap_or("by-key")));
                    if self.abort_fails {
                        return Err("abort failed");
                    }
                }
                Ok(())
            },
            || async {
                self.record("reclaim");
                if self.reclaim_fails {
                    return Err("reclaim failed");
                }
                self.recovery.store(false, Ordering::SeqCst);
                Ok::<_, &str>(true)
            },
        )
        .await;
    }
}

#[tokio::test]
async fn success_records_vendor_id_before_relay_and_never_compensates() {
    let fake = Fake::new(true, None);
    assert_eq!(initialize(&fake).await, Ok(()));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        ["create", "attach:vendor-id", "relay"]
    );
    assert!(fake.recovery.load(Ordering::SeqCst));
}

#[tokio::test]
async fn filesystem_skips_vendor_id_persistence() {
    let fake = Fake::new(false, None);
    assert_eq!(initialize(&fake).await, Ok(()));
    assert_eq!(*fake.calls.lock().unwrap(), ["create", "relay"]);
}

#[tokio::test]
async fn unknown_vendor_id_uses_key_cleanup_before_reclaim() {
    let fake = Fake::new(true, Some(Failure::Create));
    assert_eq!(initialize(&fake).await, Err(Failure::Create));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        ["create", "abort:by-key", "reclaim"]
    );
}

#[tokio::test]
async fn id_persistence_failure_passes_the_unrecorded_id_to_compensation() {
    let fake = Fake::new(true, Some(Failure::Attach));
    assert_eq!(initialize(&fake).await, Err(Failure::Attach));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        ["create", "attach:vendor-id", "abort:vendor-id", "reclaim"]
    );
}

#[tokio::test]
async fn either_relay_failure_preserves_known_vendor_handle() {
    for error in [Failure::RelaySecret, Failure::RelayWrite] {
        let fake = Fake::new(true, Some(error));
        assert_eq!(initialize(&fake).await, Err(error));
        assert_eq!(
            *fake.calls.lock().unwrap(),
            [
                "create",
                "attach:vendor-id",
                "relay",
                "abort:vendor-id",
                "reclaim"
            ]
        );
    }
}

#[tokio::test]
async fn abort_failure_retains_recovery_and_the_original_error_at_every_stage() {
    for error in [
        Failure::Create,
        Failure::Attach,
        Failure::RelaySecret,
        Failure::RelayWrite,
    ] {
        let mut fake = Fake::new(true, Some(error));
        fake.abort_fails = true;
        assert_eq!(initialize(&fake).await, Err(error));
        assert!(fake.recovery.load(Ordering::SeqCst));
        assert!(
            !fake
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|call| call == "reclaim")
        );
    }
}

#[tokio::test]
async fn filesystem_relay_failure_reclaims_without_vendor_abort() {
    let fake = Fake::new(false, Some(Failure::RelayWrite));
    assert_eq!(initialize(&fake).await, Err(Failure::RelayWrite));
    assert_eq!(*fake.calls.lock().unwrap(), ["create", "relay", "reclaim"]);
}

#[tokio::test]
async fn reclaim_failure_also_preserves_original_creation_error() {
    let mut fake = Fake::new(true, Some(Failure::Attach));
    fake.reclaim_fails = true;
    assert_eq!(initialize(&fake).await, Err(Failure::Attach));
    assert!(fake.recovery.load(Ordering::SeqCst));
    assert_eq!(
        *fake.calls.lock().unwrap(),
        ["create", "attach:vendor-id", "abort:vendor-id", "reclaim"]
    );
}
