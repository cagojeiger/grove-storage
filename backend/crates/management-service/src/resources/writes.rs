use super::PreparedCredential;
use crate::Error;
use filegate_core::Crypto;
use filegate_db::management::{AuditContext, IdentityTransaction};
use grove_management_command::{Command, Output, model};

pub(super) async fn run(
    tx: &mut IdentityTransaction<'_>,
    crypto: &Crypto,
    ctx: &AuditContext,
    command: Command,
) -> Result<Output, Error> {
    Ok(match command {
        Command::StorageMetadataReplace(input) => {
            Output::StorageMetadataReplace(model::ResourceMetadata {
                metadata: tx
                    .replace_resource_metadata(
                        ctx,
                        filegate_db::management::MetadataResource::Storage,
                        &input.id,
                        &input.metadata,
                    )
                    .await?,
                id: input.id,
            })
        }
        Command::ClientMetadataReplace(input) => {
            Output::ClientMetadataReplace(model::ResourceMetadata {
                metadata: tx
                    .replace_resource_metadata(
                        ctx,
                        filegate_db::management::MetadataResource::Client,
                        &input.id,
                        &input.metadata,
                    )
                    .await?,
                id: input.id,
            })
        }
        Command::StorageDelete(input) => {
            tx.delete_resource_storage(ctx, &input.id).await?;
            Output::StorageDelete(deleted("storage", input.id, None))
        }
        Command::ClientCreate(input) => {
            tx.create_resource_client(ctx, &input.id, &input.storage_id)
                .await?;
            Output::ClientCreate(model::Client {
                id: input.id,
                storage_id: input.storage_id,
            })
        }
        Command::ClientDelete(input) => {
            tx.delete_resource_client(ctx, &input.id).await?;
            Output::ClientDelete(deleted("client", input.id, None))
        }
        Command::ClientKeyRegister(input) => {
            tx.register_service_key(ctx, &input.client_id, &input.key_hash)
                .await?;
            Output::ClientKeyRegister(model::ClientKey {
                client_id: input.client_id,
                key_hash: input.key_hash,
            })
        }
        Command::ClientKeyDelete(input) => {
            tx.delete_service_key(ctx, &input.client_id, &input.key_hash)
                .await?;
            Output::ClientKeyDelete(deleted("client-key", input.key_hash, Some(input.client_id)))
        }
        Command::CredentialCreate(input) => {
            let prepared = PreparedCredential::new(crypto).map_err(|_| Error::Unavailable)?;
            tx.create_service_credential(ctx, &input.client_id, prepared.encrypted())
                .await?;
            Output::CredentialCreate(prepared.into_output())
        }
        Command::CredentialDelete(input) => {
            tx.delete_service_credential(ctx, &input.client_id, &input.access_key_id)
                .await?;
            Output::CredentialDelete(deleted(
                "credential",
                input.access_key_id,
                Some(input.client_id),
            ))
        }
        _ => return Err(Error::RequestRejected),
    })
}

fn deleted(resource: &str, id: String, client_id: Option<String>) -> model::Deleted {
    model::Deleted {
        resource: resource.into(),
        id,
        client_id,
    }
}
