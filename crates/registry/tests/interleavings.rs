#![allow(clippy::unwrap_used)]
mod support;
use grove_domain::References;
use grove_registry::*;
use std::sync::Arc;
use support::*;

#[tokio::test]
async fn reference_added_during_probe_is_checked_at_write_time() {
    let repo = MemoryRepository::default();
    registry(repo.clone())
        .register(id(), spec("old"), credentials())
        .await
        .unwrap();
    let hook_repo = repo.clone();
    let probe = Probe {
        on_verify: Some(Arc::new(move || {
            hook_repo
                .references(
                    &id(),
                    References {
                        uploads: 1,
                        ..Default::default()
                    },
                )
                .unwrap();
        })),
        ..Default::default()
    };
    let service = Registry::new(repo.clone(), probe, Protector::default());
    assert_eq!(
        service.replace(&id(), 1, spec("new"), credentials()).await,
        Err(Error::InUse)
    );
    assert_eq!(
        repo.get(&id()).await.unwrap().unwrap().spec.target.bucket(),
        "old"
    );
}

#[tokio::test]
async fn duplicate_insert_is_rejected_atomically_without_overwriting() {
    let repo = MemoryRepository::default();
    let protected = || ProtectedCredentials {
        key_id: "test".into(),
        payload: vec![],
    };
    let (a, b) = tokio::join!(
        repo.insert(id(), spec("a"), protected()),
        repo.insert(id(), spec("b"), protected())
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(a == Err(Error::AlreadyExists) || b == Err(Error::AlreadyExists));
}

#[tokio::test]
async fn two_updates_with_same_revision_have_one_winner() {
    let repo = MemoryRepository::default();
    let service = registry(repo.clone());
    service
        .register(id(), spec("old"), credentials())
        .await
        .unwrap();
    let storage_id = id();
    let (a, b) = tokio::join!(
        service.replace(&storage_id, 1, spec("a"), credentials()),
        service.replace(&storage_id, 1, spec("b"), credentials())
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(a == Err(Error::ConcurrentChange) || b == Err(Error::ConcurrentChange));
}
