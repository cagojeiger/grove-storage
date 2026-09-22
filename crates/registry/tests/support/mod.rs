#![allow(clippy::unwrap_used, dead_code)]

use grove_domain::*;
use grove_registry::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct State {
    rows: HashMap<StorageId, (Storage, ProtectedCredentials, References)>,
    fail_write: bool,
    fail_read: bool,
    revision: u64,
}

#[derive(Clone, Default)]
pub struct MemoryRepository(Arc<Mutex<State>>);

impl MemoryRepository {
    pub fn fail_write(&self) {
        self.0.lock().unwrap().fail_write = true;
    }
    pub fn fail_read(&self) {
        self.0.lock().unwrap().fail_read = true;
    }
    pub fn references(&self, id: &StorageId, references: References) -> Result<(), Error> {
        let mut state = self.0.lock().unwrap();
        let (_, _, current) = state.rows.get_mut(id).ok_or(Error::NotFound)?;
        *current = references;
        Ok(())
    }
    pub fn protected(&self, id: &StorageId) -> ProtectedCredentials {
        self.0.lock().unwrap().rows.get(id).unwrap().1.clone()
    }
}

impl StorageRepository for MemoryRepository {
    async fn get(&self, id: &StorageId) -> Result<Option<Storage>, Error> {
        let state = self.0.lock().unwrap();
        if state.fail_read {
            return Err(Error::PersistenceFailed);
        }
        Ok(state.rows.get(id).map(|(storage, _, _)| storage.clone()))
    }
    async fn insert(
        &self,
        id: StorageId,
        spec: StorageSpec,
        credentials: ProtectedCredentials,
    ) -> Result<Storage, Error> {
        let mut state = self.0.lock().unwrap();
        if state.fail_write {
            return Err(Error::PersistenceFailed);
        }
        if state.rows.contains_key(&id) {
            return Err(Error::AlreadyExists);
        }
        state.revision += 1;
        let storage = Storage {
            id: id.clone(),
            spec,
            revision: state.revision,
        };
        state
            .rows
            .insert(id, (storage.clone(), credentials, References::default()));
        Ok(storage)
    }
    async fn replace(
        &self,
        id: &StorageId,
        revision: u64,
        spec: StorageSpec,
        credentials: ProtectedCredentials,
    ) -> Result<Storage, Error> {
        let mut state = self.0.lock().unwrap();
        if state.fail_write {
            return Err(Error::PersistenceFailed);
        }
        let (current, _, references) = state.rows.get(id).ok_or(Error::NotFound)?;
        if current.revision != revision {
            return Err(Error::ConcurrentChange);
        }
        if !references.permits_replace(&current.spec, &spec) {
            return Err(Error::InUse);
        }
        state.revision += 1;
        let next_revision = state.revision;
        let (current, protected, _) = state.rows.get_mut(id).ok_or(Error::NotFound)?;
        current.revision = next_revision;
        current.spec = spec;
        *protected = credentials;
        Ok(current.clone())
    }
    async fn delete_if_unreferenced(&self, id: &StorageId) -> Result<bool, Error> {
        let mut state = self.0.lock().unwrap();
        if state.fail_write {
            return Err(Error::PersistenceFailed);
        }
        if state
            .rows
            .get(id)
            .is_some_and(|(_, _, refs)| !refs.permits_delete())
        {
            return Err(Error::InUse);
        }
        Ok(state.rows.remove(id).is_some())
    }
}

#[derive(Clone, Default)]
pub struct Probe {
    pub failure: bool,
    pub calls: Arc<Mutex<usize>>,
    pub on_verify: Option<Arc<dyn Fn() + Send + Sync>>,
}
impl StorageProbe for Probe {
    async fn verify(&self, _: &StorageSpec, _: &ProviderCredentials) -> Result<(), Error> {
        *self.calls.lock().unwrap() += 1;
        if let Some(hook) = &self.on_verify {
            hook();
        }
        if self.failure {
            Err(Error::ProbeFailed)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
pub struct Protector {
    pub failure: bool,
}
impl CredentialProtector for Protector {
    fn protect(
        &self,
        id: &StorageId,
        _: &ProviderCredentials,
    ) -> Result<ProtectedCredentials, Error> {
        if self.failure {
            return Err(Error::ProtectionFailed);
        }
        // Not encryption. This fake only proves that plaintext is not sent to persistence.
        Ok(ProtectedCredentials {
            key_id: "test-only".into(),
            payload: id.as_str().as_bytes().to_vec(),
        })
    }
}

pub fn id() -> StorageId {
    StorageId::parse("s3-primary").unwrap()
}
pub fn spec(bucket: &str) -> StorageSpec {
    let endpoint = Endpoint::parse("https://s3.example.com").unwrap();
    StorageSpec {
        target: S3Target::new(endpoint.clone(), "auto".into(), bucket.into(), false).unwrap(),
        public_endpoint: endpoint,
        capacity: Capacity::new(1000).unwrap(),
        force_relay: false,
    }
}
pub fn credentials() -> ProviderCredentials {
    ProviderCredentials {
        access_key: "test-access".into(),
        secret_key: "test-secret".into(),
    }
}
pub fn registry(repo: MemoryRepository) -> Registry<MemoryRepository, Probe, Protector> {
    Registry::new(repo, Probe::default(), Protector::default())
}
