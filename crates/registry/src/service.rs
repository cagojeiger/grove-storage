use crate::{
    CredentialProtector, Error, ExposeSecret, ProviderCredentials, Storage, StorageProbe,
    StorageRepository,
};
use grove_domain::{StorageId, StorageSpec};

pub struct Registry<R, P, C> {
    repository: R,
    probe: P,
    protector: C,
}

impl<R: StorageRepository, P: StorageProbe, C: CredentialProtector> Registry<R, P, C> {
    pub fn new(repository: R, probe: P, protector: C) -> Self {
        Self {
            repository,
            probe,
            protector,
        }
    }

    pub async fn register(
        &self,
        id: StorageId,
        spec: StorageSpec,
        credentials: ProviderCredentials,
    ) -> Result<Storage, Error> {
        validate_credentials(&credentials)?;
        if self.repository.get(&id).await?.is_some() {
            return Err(Error::AlreadyExists);
        }
        self.probe.verify(&spec, &credentials).await?;
        let protected = self.protector.protect(&id, &credentials)?;
        self.repository.insert(id, spec, protected).await
    }

    pub async fn replace(
        &self,
        id: &StorageId,
        expected_revision: u64,
        spec: StorageSpec,
        credentials: ProviderCredentials,
    ) -> Result<Storage, Error> {
        validate_credentials(&credentials)?;
        let current = self.repository.get(id).await?.ok_or(Error::NotFound)?;
        if current.revision != expected_revision {
            return Err(Error::ConcurrentChange);
        }
        self.probe.verify(&spec, &credentials).await?;
        let protected = self.protector.protect(id, &credentials)?;
        self.repository
            .replace(id, expected_revision, spec, protected)
            .await
    }

    pub async fn get(&self, id: &StorageId) -> Result<Storage, Error> {
        self.repository.get(id).await?.ok_or(Error::NotFound)
    }

    pub async fn delete(&self, id: &StorageId) -> Result<bool, Error> {
        self.repository.delete_if_unreferenced(id).await
    }
}

fn validate_credentials(credentials: &ProviderCredentials) -> Result<(), Error> {
    if credentials.access_key.expose_secret().is_empty()
        || credentials.secret_key.expose_secret().is_empty()
    {
        return Err(Error::InvalidCredentials);
    }
    Ok(())
}
