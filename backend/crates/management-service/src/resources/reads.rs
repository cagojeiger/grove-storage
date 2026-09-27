use super::{client_usage_output, snapshot_output, storage_output, storage_usage_output};
use crate::Error;
use filegate_db::management::IdentityTransaction;
use grove_management_command::{
    Command, Output,
    model::{self, State},
};

pub(super) async fn run(
    tx: &mut IdentityTransaction<'_>,
    command: Command,
) -> Result<Output, Error> {
    Ok(match command {
        Command::StorageMetadataShow(input) => {
            Output::StorageMetadataShow(model::ResourceMetadata {
                metadata: tx
                    .resource_metadata(
                        filegate_db::management::MetadataResource::Storage,
                        &input.id,
                    )
                    .await?,
                id: input.id,
            })
        }
        Command::ClientMetadataShow(input) => Output::ClientMetadataShow(model::ResourceMetadata {
            metadata: tx
                .resource_metadata(filegate_db::management::MetadataResource::Client, &input.id)
                .await?,
            id: input.id,
        }),
        Command::StorageList(_) => Output::StorageList(
            tx.storages()
                .await?
                .into_iter()
                .map(storage_output)
                .collect::<Result<_, _>>()?,
        ),
        Command::StorageShow(input) => {
            Output::StorageShow(storage_output(tx.storage(&input.id).await?)?)
        }
        Command::ClientList(_) => Output::ClientList(tx.clients().await?),
        Command::ClientShow(input) => Output::ClientShow(model::Client {
            storage_id: tx.client_storage(&input.id).await?,
            id: input.id,
        }),
        Command::CredentialList(input) => {
            Output::CredentialList(tx.service_credentials(&input.client_id).await?)
        }
        Command::ClientKeyList(input) => {
            Output::ClientKeyList(tx.client_keys(&input.client_id).await?)
        }
        Command::UsageStorages(_) => Output::UsageStorages(
            tx.storage_usage()
                .await?
                .into_iter()
                .map(storage_usage_output)
                .collect::<Result<_, Error>>()?,
        ),
        Command::UsageClients(_) => Output::UsageClients(
            tx.client_usage()
                .await?
                .into_iter()
                .map(client_usage_output)
                .collect(),
        ),
        Command::UsageHistory(input) => Output::UsageHistory(
            tx.usage_history(input.days)
                .await?
                .into_iter()
                .map(snapshot_output)
                .collect(),
        ),
        Command::Status(_) => {
            // Server-side DB observations, not client network/storage probes.
            let storages = tx.storage_usage().await?;
            let clients = tx.clients().await?;
            Output::Status(model::Status {
                server_version: Some(env!("CARGO_PKG_VERSION").into()),
                identity: State::Ok,
                health: State::Ok,
                readiness: State::Ok,
                registry: model::Registry {
                    state: State::Ok,
                    usage: State::Ok,
                    clients: State::Ok,
                    storage_count: Some(storages.len()),
                    client_count: Some(clients.len()),
                },
                storage_access: model::StorageAccess::NotChecked,
            })
        }
        _ => return Err(Error::RequestRejected),
    })
}
