//! Storage configuration values. No runtime, database, or provider SDK dependency.

use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Invalid {
    #[error("storage ID must be a lowercase alphanumeric slug of 1..=63 bytes")]
    StorageId,
    #[error("endpoint must be an HTTP(S) URL without credentials, query, or fragment")]
    Endpoint,
    #[error("region must not be empty or contain whitespace")]
    Region,
    #[error("bucket must not be empty or contain whitespace or slashes")]
    Bucket,
    #[error("capacity must be nonnegative; zero means unlimited")]
    Capacity,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorageId(String);

impl StorageId {
    pub fn parse(value: impl Into<String>) -> Result<Self, Invalid> {
        let value = value.into();
        let edge_ok = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
        if !(1..=63).contains(&value.len())
            || !value.bytes().all(|b| edge_ok(b) || b == b'-')
            || !value.bytes().next().is_some_and(edge_ok)
            || !value.bytes().last().is_some_and(edge_ok)
        {
            return Err(Invalid::StorageId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint(Url);

impl Endpoint {
    pub fn parse(value: &str) -> Result<Self, Invalid> {
        let url = Url::parse(value).map_err(|_| Invalid::Endpoint)?;
        if value.trim() != value
            || value.chars().any(char::is_control)
            || !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Invalid::Endpoint);
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity(i64);

impl Capacity {
    pub fn new(bytes: i64) -> Result<Self, Invalid> {
        if bytes < 0 {
            return Err(Invalid::Capacity);
        }
        Ok(Self(bytes))
    }
    pub fn bytes(self) -> i64 {
        self.0
    }
}

/// Conservatively treats routing changes as physical target changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Target {
    endpoint: Endpoint,
    region: String,
    bucket: String,
    path_style: bool,
}

impl S3Target {
    pub fn new(
        endpoint: Endpoint,
        region: String,
        bucket: String,
        path_style: bool,
    ) -> Result<Self, Invalid> {
        if region.is_empty() || region.chars().any(char::is_whitespace) {
            return Err(Invalid::Region);
        }
        if bucket.is_empty()
            || bucket
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || c == '/' || c == '\\')
        {
            return Err(Invalid::Bucket);
        }
        Ok(Self {
            endpoint,
            region,
            bucket,
            path_style,
        })
    }
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
    pub fn region(&self) -> &str {
        &self.region
    }
    pub fn bucket(&self) -> &str {
        &self.bucket
    }
    pub fn path_style(&self) -> bool {
        self.path_style
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSpec {
    pub target: S3Target,
    pub public_endpoint: Endpoint,
    pub capacity: Capacity,
    pub force_relay: bool,
}

impl StorageSpec {
    pub fn changes_physical_target(&self, next: &Self) -> bool {
        self.target != next.target
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct References {
    pub clients: u64,
    pub files: u64,
    pub uploads: u64,
    pub cleanup_jobs: u64,
}

impl References {
    pub fn permits_delete(self) -> bool {
        self.clients == 0 && self.files == 0 && self.uploads == 0 && self.cleanup_jobs == 0
    }

    pub fn permits_replace(self, current: &StorageSpec, next: &StorageSpec) -> bool {
        !current.changes_physical_target(next) || self.permits_delete()
    }
}
