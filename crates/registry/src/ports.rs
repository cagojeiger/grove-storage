use crate::{Error, ProtectedCredentials, ProviderCredentials, Storage};
use grove_domain::{StorageId, StorageSpec};

pub trait StorageRepository {
    async fn get(&self, id: &StorageId) -> Result<Option<Storage>, Error>;

    /// Atomically creates a fresh revision or returns AlreadyExists, never overwrites.
    /// Revisions must not be reused after deletion and recreation of an ID.
    async fn insert(
        &self,
        id: StorageId,
        spec: StorageSpec,
        credentials: ProtectedCredentials,
    ) -> Result<Storage, Error>;

    /// In one transaction: check revision, compare the current physical target,
    /// reject target changes if any client/file/upload/cleanup reference exists,
    /// then replace spec and credentials and advance the revision.
    /// Reference creation must serialize with this operation.
    async fn replace(
        &self,
        id: &StorageId,
        expected_revision: u64,
        spec: StorageSpec,
        credentials: ProtectedCredentials,
    ) -> Result<Storage, Error>;

    /// Atomically checks all references and deletes. Absence is success (false).
    /// Must serialize with reference creation; a prior reference-count query is insufficient.
    async fn delete_if_unreferenced(&self, id: &StorageId) -> Result<bool, Error>;
}

pub trait StorageProbe {
    /// Read-only access check. Does not create a bucket or prove all object I/O permissions.
    async fn verify(
        &self,
        spec: &StorageSpec,
        credentials: &ProviderCredentials,
    ) -> Result<(), Error>;
}

pub trait CredentialProtector {
    /// Binds ciphertext to storage identity. Never persists or logs the plaintext.
    fn protect(
        &self,
        id: &StorageId,
        credentials: &ProviderCredentials,
    ) -> Result<ProtectedCredentials, Error>;
}
