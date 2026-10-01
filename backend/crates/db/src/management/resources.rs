//! Resource reads share the identity fence and the existing registry SQL.
use super::{Error, IdentityTransaction};
use crate::{registry, s3_registry, usage};

impl IdentityTransaction<'_> {
    pub async fn storages(&mut self) -> Result<Vec<registry::StorageRow>, Error> {
        Ok(registry::list_storages(&mut *self.inner).await?)
    }
    pub async fn storage(&mut self, id: &str) -> Result<registry::StorageRow, Error> {
        registry::get_storage(&mut *self.inner, id)
            .await?
            .ok_or(Error::NotFound)
    }
    pub async fn clients(&mut self) -> Result<Vec<String>, Error> {
        Ok(registry::list_clients(&mut *self.inner).await?)
    }
    pub async fn client_storage(&mut self, id: &str) -> Result<String, Error> {
        registry::client_storage(&mut *self.inner, id)
            .await?
            .ok_or(Error::NotFound)
    }
    pub async fn client_keys(&mut self, id: &str) -> Result<Vec<String>, Error> {
        self.client_storage(id).await?;
        Ok(registry::list_client_keys(&mut *self.inner, id).await?)
    }
    pub async fn service_credentials(&mut self, id: &str) -> Result<Vec<String>, Error> {
        self.client_storage(id).await?;
        Ok(s3_registry::list_credentials(&mut *self.inner, id).await?)
    }
    pub async fn storage_usage(&mut self) -> Result<Vec<usage::StorageUsage>, Error> {
        Ok(usage::by_storage(&mut *self.inner).await?)
    }
    pub async fn client_usage(&mut self) -> Result<Vec<usage::ClientUsage>, Error> {
        Ok(usage::by_client(&mut *self.inner).await?)
    }
    pub async fn usage_history(&mut self, days: u16) -> Result<Vec<usage::SnapshotRow>, Error> {
        Ok(usage::snapshot_history(&mut *self.inner, i32::from(days)).await?)
    }
}
