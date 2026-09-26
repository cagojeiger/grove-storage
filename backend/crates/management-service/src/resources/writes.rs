use crate::Error;
use filegate_core::{Crypto, ExposeSecret, SecretString};
use filegate_db::management::{AuditContext, EncryptedServiceCredential, IdentityTransaction};
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
            let access_key_id = filegate_core::generate_access_key_id();
            let secret = SecretString::from(filegate_core::generate_url_secret());
            let encrypted = crypto
                .encrypt(&access_key_id, &secret)
                .map_err(|_| Error::Unavailable)?;
            tx.create_service_credential(
                ctx,
                &input.client_id,
                EncryptedServiceCredential {
                    access_key_id: &access_key_id,
                    ciphertext: &encrypted.ciphertext,
                    nonce: &encrypted.nonce,
                    enc_key_id: crypto.active_key_id(),
                },
            )
            .await?;
            Output::CredentialCreate(model::IssuedCredential {
                access_key_id,
                secret_key: secret.expose_secret().to_owned(),
            })
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
