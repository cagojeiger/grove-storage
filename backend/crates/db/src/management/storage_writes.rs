//! Storage registry changes and bounded, secret-free audit snapshots.
use super::{AuditContext, Error, IdentityTransaction, audit};
use crate::registry::{self, StorageRow, UpdateStorageOutcome};

impl IdentityTransaction<'_> {
    pub async fn create_resource_storage(
        &mut self,
        ctx: &AuditContext,
        row: &StorageRow,
    ) -> Result<(), Error> {
        registry::insert_storage(&mut *self.inner, row).await?;
        self.storage_audit(ctx, "storage.create", &row.id, None, Some(row))
            .await
    }

    pub async fn replace_resource_storage(
        &mut self,
        ctx: &AuditContext,
        row: &StorageRow,
    ) -> Result<UpdateStorageOutcome, Error> {
        let before = registry::lock_storage(&mut self.inner, &row.id).await?;
        let outcome = registry::update_storage_in(&mut self.inner, row).await?;
        if outcome == UpdateStorageOutcome::Updated {
            self.storage_audit(ctx, "storage.replace", &row.id, before.as_ref(), Some(row))
                .await?;
        }
        Ok(outcome)
    }

    pub async fn delete_resource_storage(
        &mut self,
        ctx: &AuditContext,
        id: &str,
    ) -> Result<(), Error> {
        let before = registry::lock_storage(&mut self.inner, id).await?;
        if registry::delete_storage_rows(&mut *self.inner, id).await? > 0 {
            self.storage_audit(ctx, "storage.delete", id, before.as_ref(), None)
                .await?;
        }
        Ok(())
    }

    async fn storage_audit(
        &mut self,
        ctx: &AuditContext,
        action: &str,
        storage: &str,
        before: Option<&StorageRow>,
        after: Option<&StorageRow>,
    ) -> Result<(), Error> {
        let id = audit::record_target(&mut self.inner, ctx, action, "storage", storage).await?;
        // Addresses can contain user-supplied credentials; retain only change flags.
        let address_changed = before
            .zip(after)
            .is_some_and(|(a, b)| registry::storage_address_changed(a, b));
        sqlx::query("UPDATE management.audit_events SET metadata=jsonb_build_object(
            'before', CASE WHEN $2::text IS NULL THEN NULL ELSE jsonb_build_object('kind',$2::text,'capacity_bytes',$3::bigint,'force_relay',$4::bool) END,
            'after', CASE WHEN $5::text IS NULL THEN NULL ELSE jsonb_build_object('kind',$5::text,'capacity_bytes',$6::bigint,'force_relay',$7::bool) END,
            'address_changed',$8::bool) WHERE id=$1")
            .bind(id).bind(before.map(|r| r.kind.as_str())).bind(before.map(|r| r.capacity_bytes))
            .bind(before.map(|r| r.force_relay)).bind(after.map(|r| r.kind.as_str()))
            .bind(after.map(|r| r.capacity_bytes)).bind(after.map(|r| r.force_relay))
            .bind(address_changed).execute(&mut *self.inner).await?;
        Ok(())
    }
}
