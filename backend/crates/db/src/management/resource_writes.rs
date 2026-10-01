//! Resource mutations and allowlisted audit metadata share the identity transaction.
use super::{AuditContext, Error, IdentityTransaction, audit};
use crate::{registry, s3_registry};

pub struct EncryptedServiceCredential<'a> {
    pub access_key_id: &'a str,
    pub ciphertext: &'a [u8],
    pub nonce: &'a [u8],
    pub enc_key_id: &'a str,
}

struct Event<'a> {
    action: &'static str,
    kind: &'static str,
    id: &'a str,
    client: &'a str,
    storage: Option<&'a str>,
}

impl IdentityTransaction<'_> {
    pub async fn create_resource_client(
        &mut self,
        ctx: &AuditContext,
        id: &str,
        storage: &str,
    ) -> Result<(), Error> {
        if registry::RESERVED_CLIENT_IDS.contains(&id) {
            return Err(Error::InvalidInput);
        }
        registry::insert_client(&mut *self.inner, id, storage)
            .await
            .map_err(insert_error)?;
        self.resource_audit(
            ctx,
            Event {
                action: "client.create",
                kind: "client",
                id,
                client: id,
                storage: Some(storage),
            },
        )
        .await
    }

    pub async fn delete_resource_client(
        &mut self,
        ctx: &AuditContext,
        id: &str,
    ) -> Result<(), Error> {
        if registry::delete_client_rows(&mut *self.inner, id).await? > 0 {
            self.resource_audit(
                ctx,
                Event {
                    action: "client.delete",
                    kind: "client",
                    id,
                    client: id,
                    storage: None,
                },
            )
            .await?;
        }
        Ok(())
    }

    pub async fn register_service_key(
        &mut self,
        ctx: &AuditContext,
        client: &str,
        hash: &str,
    ) -> Result<(), Error> {
        registry::insert_client_key(&mut *self.inner, client, hash)
            .await
            .map_err(insert_error)?;
        self.resource_audit(
            ctx,
            Event {
                action: "client-key.register",
                kind: "client_key",
                id: client,
                client,
                storage: None,
            },
        )
        .await
    }

    pub async fn delete_service_key(
        &mut self,
        ctx: &AuditContext,
        client: &str,
        hash: &str,
    ) -> Result<(), Error> {
        if registry::delete_client_key_rows(&mut *self.inner, client, hash).await? > 0 {
            self.resource_audit(
                ctx,
                Event {
                    action: "client-key.delete",
                    kind: "client_key",
                    id: client,
                    client,
                    storage: None,
                },
            )
            .await?;
        }
        Ok(())
    }

    pub async fn create_service_credential(
        &mut self,
        ctx: &AuditContext,
        client: &str,
        key: EncryptedServiceCredential<'_>,
    ) -> Result<(), Error> {
        s3_registry::insert_credential(
            &mut *self.inner,
            key.access_key_id,
            client,
            key.ciphertext,
            key.nonce,
            key.enc_key_id,
        )
        .await
        .map_err(insert_error)?;
        self.resource_audit(
            ctx,
            Event {
                action: "credential.create",
                kind: "s3_credential",
                id: key.access_key_id,
                client,
                storage: None,
            },
        )
        .await
    }

    pub async fn delete_service_credential(
        &mut self,
        ctx: &AuditContext,
        client: &str,
        id: &str,
    ) -> Result<(), Error> {
        if s3_registry::delete_credential(&mut *self.inner, client, id).await? > 0 {
            self.resource_audit(
                ctx,
                Event {
                    action: "credential.delete",
                    kind: "s3_credential",
                    id,
                    client,
                    storage: None,
                },
            )
            .await?;
        }
        Ok(())
    }

    async fn resource_audit(&mut self, ctx: &AuditContext, event: Event<'_>) -> Result<(), Error> {
        let id =
            audit::record_target(&mut self.inner, ctx, event.action, event.kind, event.id).await?;
        sqlx::query("UPDATE management.audit_events SET metadata=jsonb_strip_nulls(jsonb_build_object('client_id',$2::text,'storage_id',$3::text)) WHERE id=$1")
            .bind(id).bind(event.client).bind(event.storage).execute(&mut *self.inner).await?;
        Ok(())
    }
}

fn insert_error(error: sqlx::Error) -> Error {
    match registry::write_violation(&error, registry::WriteOp::Insert) {
        Some(registry::WriteViolation::MissingRef(_) | registry::WriteViolation::Invalid) => {
            Error::InvalidInput
        }
        _ => error.into(),
    }
}
