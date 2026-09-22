#![allow(clippy::unwrap_used)]
mod support;
use grove_domain::{Capacity, References};
use grove_registry::*;

#[tokio::test]
async fn recreated_id_does_not_accept_a_previous_incarnations_revision() {
    let service = registry(MemoryRepository::default());
    let old = service
        .register(id(), spec("old"), credentials())
        .await
        .unwrap();
    service.delete(&id()).await.unwrap();
    let new = service
        .register(id(), spec("new"), credentials())
        .await
        .unwrap();
    assert_ne!(old.revision, new.revision);
    assert_eq!(
        service
            .replace(&id(), old.revision, spec("old"), credentials())
            .await,
        Err(Error::ConcurrentChange)
    );
}

#[tokio::test]
async fn failed_replacement_preserves_existing_spec_and_credentials() {
    let repo = MemoryRepository::default();
    registry(repo.clone())
        .register(id(), spec("old"), credentials())
        .await
        .unwrap();
    let previous = repo.protected(&id()).payload;
    let service = Registry::new(
        repo.clone(),
        Probe {
            failure: true,
            ..Default::default()
        },
        Protector::default(),
    );
    assert_eq!(
        service.replace(&id(), 1, spec("new"), credentials()).await,
        Err(Error::ProbeFailed)
    );
    assert_eq!(
        repo.get(&id()).await.unwrap().unwrap().spec.target.bucket(),
        "old"
    );
    assert_eq!(repo.protected(&id()).payload, previous);
}
use support::*;

#[tokio::test]
async fn referenced_storage_allows_metadata_update_but_not_retarget_or_delete() {
    let repo = MemoryRepository::default();
    let service = registry(repo.clone());
    service
        .register(id(), spec("old"), credentials())
        .await
        .unwrap();
    repo.references(
        &id(),
        References {
            files: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        service.replace(&id(), 1, spec("new"), credentials()).await,
        Err(Error::InUse)
    );
    assert_eq!(service.delete(&id()).await, Err(Error::InUse));
    let mut next = spec("old");
    next.capacity = Capacity::new(0).unwrap();
    assert_eq!(
        service
            .replace(&id(), 1, next, credentials())
            .await
            .unwrap()
            .revision,
        2
    );
}

#[tokio::test]
async fn unreferenced_storage_can_be_retargeted_and_deleted_idempotently() {
    let service = registry(MemoryRepository::default());
    service
        .register(id(), spec("old"), credentials())
        .await
        .unwrap();
    assert_eq!(
        service
            .replace(&id(), 1, spec("new"), credentials())
            .await
            .unwrap()
            .revision,
        2
    );
    assert!(service.delete(&id()).await.unwrap());
    assert!(!service.delete(&id()).await.unwrap());
}

#[tokio::test]
async fn stale_revision_and_missing_storage_fail_before_probe() {
    let repo = MemoryRepository::default();
    let probe = Probe::default();
    let calls = probe.calls.clone();
    let service = Registry::new(repo, probe, Protector::default());
    assert_eq!(
        service.replace(&id(), 1, spec("old"), credentials()).await,
        Err(Error::NotFound)
    );
    service
        .register(id(), spec("old"), credentials())
        .await
        .unwrap();
    assert_eq!(
        service.replace(&id(), 2, spec("new"), credentials()).await,
        Err(Error::ConcurrentChange)
    );
    assert_eq!(*calls.lock().unwrap(), 1);
}
