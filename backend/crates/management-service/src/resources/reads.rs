use crate::Error;
use filegate_db::{management::IdentityTransaction, registry::StorageRow};
use grove_management_command::{
    Command, Output,
    model::{self, State, StorageKind},
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
                .map(storage)
                .collect::<Result<_, _>>()?,
        ),
        Command::StorageShow(input) => Output::StorageShow(storage(tx.storage(&input.id).await?)?),
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
                .map(|r| {
                    Ok(model::StorageUsage {
                        storage_id: r.storage_id,
                        kind: kind(&r.kind)?,
                        capacity_bytes: r.capacity_bytes,
                        reserved_bytes: r.reserved_bytes,
                        active_bytes: r.active_bytes,
                        purge_pending_bytes: r.purge_pending_bytes,
                        remaining_bytes: r
                            .capacity_bytes
                            .checked_sub(r.reserved_bytes)
                            .and_then(|n| n.checked_sub(r.active_bytes))
                            .and_then(|n| n.checked_sub(r.purge_pending_bytes))
                            .ok_or(Error::Unavailable)?,
                        reserved_files: r.reserved_files,
                        active_files: r.active_files,
                        purge_pending_files: r.purge_pending_files,
                    })
                })
                .collect::<Result<_, Error>>()?,
        ),
        Command::UsageClients(_) => Output::UsageClients(
            tx.client_usage()
                .await?
                .into_iter()
                .map(|r| model::ClientUsage {
                    client_id: r.client_id,
                    storage_id: r.storage_id,
                    active_files: r.active_files,
                    active_bytes: r.active_bytes,
                })
                .collect(),
        ),
        Command::UsageHistory(input) => Output::UsageHistory(
            tx.usage_history(input.days)
                .await?
                .into_iter()
                .map(|r| model::Snapshot {
                    day: r.day.to_string(),
                    storage_id: r.storage_id,
                    client_id: r.client_id,
                    active_bytes: r.active_bytes,
                    active_files: r.active_files,
                })
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

fn kind(value: &str) -> Result<StorageKind, Error> {
    match value {
        "s3" => Ok(StorageKind::S3),
        "fs" => Ok(StorageKind::Fs),
        _ => Err(Error::Unavailable),
    }
}
pub(super) fn storage(r: StorageRow) -> Result<model::Storage, Error> {
    Ok(model::Storage {
        id: r.id,
        kind: kind(&r.kind)?,
        force_relay: r.force_relay,
        root_path: r.root_path,
        endpoint: r.endpoint,
        public_endpoint: r.public_endpoint,
        region: r.region,
        bucket: r.bucket,
        force_path_style: r.force_path_style,
        access_key: r.access_key,
        capacity_bytes: r.capacity_bytes,
    })
}
