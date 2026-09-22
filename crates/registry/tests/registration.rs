#![allow(clippy::unwrap_used)]
mod support;
use grove_registry::*;
use support::*;

#[tokio::test]
async fn registration_persists_only_protected_credentials() {
    let repo = MemoryRepository::default();
    let service = registry(repo.clone());
    let storage = service
        .register(id(), spec("bucket"), credentials())
        .await
        .unwrap();
    assert_eq!(storage.revision, 1);
    assert_eq!(service.get(&id()).await.unwrap(), storage);
    assert_ne!(repo.protected(&id()).payload, b"test-secret");
    assert!(!format!("{:?}", credentials()).contains("test-secret"));
    assert!(!format!("{:?}", repo.protected(&id())).contains("s3-primary"));
}

#[tokio::test]
async fn probe_or_encryption_failure_creates_no_record() {
    for (probe_failure, protection_failure, expected) in [
        (true, false, Error::ProbeFailed),
        (false, true, Error::ProtectionFailed),
    ] {
        let repo = MemoryRepository::default();
        let service = Registry::new(
            repo.clone(),
            Probe {
                failure: probe_failure,
                ..Default::default()
            },
            Protector {
                failure: protection_failure,
            },
        );
        assert_eq!(
            service.register(id(), spec("bucket"), credentials()).await,
            Err(expected)
        );
        assert!(repo.get(&id()).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn persistence_failure_is_not_reported_as_success_or_retried() {
    let repo = MemoryRepository::default();
    repo.fail_write();
    let probe = Probe::default();
    let calls = probe.calls.clone();
    let service = Registry::new(repo.clone(), probe, Protector::default());
    assert_eq!(
        service.register(id(), spec("bucket"), credentials()).await,
        Err(Error::PersistenceFailed)
    );
    assert_eq!(*calls.lock().unwrap(), 1);
    assert!(repo.get(&id()).await.unwrap().is_none());
}

#[tokio::test]
async fn empty_credentials_and_duplicate_ids_fail_before_probe() {
    let repo = MemoryRepository::default();
    let probe = Probe::default();
    let calls = probe.calls.clone();
    let service = Registry::new(repo, probe, Protector::default());
    let mut empty = credentials();
    empty.secret_key = "".into();
    assert_eq!(
        service.register(id(), spec("bucket"), empty).await,
        Err(Error::InvalidCredentials)
    );
    assert_eq!(*calls.lock().unwrap(), 0);
    service
        .register(id(), spec("bucket"), credentials())
        .await
        .unwrap();
    assert_eq!(
        service.register(id(), spec("other"), credentials()).await,
        Err(Error::AlreadyExists)
    );
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn read_failure_stops_before_provider_access() {
    let repo = MemoryRepository::default();
    repo.fail_read();
    let probe = Probe::default();
    let calls = probe.calls.clone();
    let service = Registry::new(repo, probe, Protector::default());
    assert_eq!(
        service.register(id(), spec("bucket"), credentials()).await,
        Err(Error::PersistenceFailed)
    );
    assert_eq!(*calls.lock().unwrap(), 0);
}
