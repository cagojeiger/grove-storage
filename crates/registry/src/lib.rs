//! Storage use cases, statically composed with provider and persistence ports.
#![allow(async_fn_in_trait)]

mod ports;
mod service;

use grove_domain::{StorageId, StorageSpec};
pub use ports::*;
pub use secrecy::{ExposeSecret, SecretString};
pub use service::Registry;

pub struct ProviderCredentials {
    pub access_key: SecretString,
    pub secret_key: SecretString,
}

impl std::fmt::Debug for ProviderCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProviderCredentials([REDACTED])")
    }
}

/// Opaque encrypted payload. The production codec and legacy import are separate adapters.
#[derive(Clone)]
pub struct ProtectedCredentials {
    pub key_id: String,
    pub payload: Vec<u8>,
}

impl std::fmt::Debug for ProtectedCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProtectedCredentials([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storage {
    pub id: StorageId,
    pub spec: StorageSpec,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("storage already exists")]
    AlreadyExists,
    #[error("storage not found")]
    NotFound,
    #[error("storage still has references")]
    InUse,
    #[error("storage changed; reload before retrying")]
    ConcurrentChange,
    #[error("provider credentials must not be empty")]
    InvalidCredentials,
    #[error("provider access verification failed")]
    ProbeFailed,
    #[error("credential protection failed")]
    ProtectionFailed,
    #[error("persistence failed; mutation outcome may be unknown")]
    PersistenceFailed,
}
