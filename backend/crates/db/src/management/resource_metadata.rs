use super::{AuditContext, Error, IdentityTransaction, audit};
use serde_json::Value;

#[derive(Clone, Copy)]
pub enum MetadataResource {
    Storage,
    Client,
}

impl IdentityTransaction<'_> {
    pub async fn resource_metadata(
        &mut self,
        resource: MetadataResource,
        id: &str,
    ) -> Result<Value, Error> {
        let sql = match resource {
            MetadataResource::Storage => "SELECT metadata FROM storages WHERE id=$1",
            MetadataResource::Client => "SELECT metadata FROM clients WHERE id=$1",
        };
        sqlx::query_scalar(sql)
            .bind(id)
            .fetch_optional(&mut *self.inner)
            .await?
            .ok_or(Error::NotFound)
    }

    pub async fn replace_resource_metadata(
        &mut self,
        ctx: &AuditContext,
        resource: MetadataResource,
        id: &str,
        metadata: &Value,
    ) -> Result<Value, Error> {
        let (sql, kind, action) = match resource {
            MetadataResource::Storage => (
                "UPDATE storages SET metadata=$2, updated_at=grove_time.transaction_now() WHERE id=$1 RETURNING metadata",
                "storage",
                "storage.metadata.replace",
            ),
            MetadataResource::Client => (
                "UPDATE clients SET metadata=$2 WHERE id=$1 RETURNING metadata",
                "client",
                "client.metadata.replace",
            ),
        };
        let result = sqlx::query_scalar(sql)
            .bind(id)
            .bind(metadata)
            .fetch_optional(&mut *self.inner)
            .await?
            .ok_or(Error::NotFound)?;
        // User-supplied labels may contain sensitive text: audit the action, not its payload.
        audit::record_target(&mut self.inner, ctx, action, kind, id).await?;
        Ok(result)
    }
}
