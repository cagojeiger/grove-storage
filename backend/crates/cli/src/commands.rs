mod result;
mod write;

use crate::args::{
    ClientCommand, ClientKeyCommand, Command, CredentialCommand, MetadataCommand, StorageCommand,
    Usage,
};
use crate::{error::Error, http::Api, model::Data};
use grove_management_command::{Command as Remote, input::*};

pub(super) type CommandResult = Result<(Data, Option<Error>), Error>;

pub async fn run(api: &Api, command: &Command) -> CommandResult {
    let command = match command {
        Command::Update { .. } | Command::Install { .. } => {
            return Err(Error::input(
                "Local commands execute without the management API",
            ));
        }
        Command::Status => Remote::Status(EmptyInput {}),
        Command::Storage(StorageCommand::List) => Remote::StorageList(EmptyInput {}),
        Command::Storage(StorageCommand::Metadata(input)) => metadata(api, input, true)?,
        Command::Client(ClientCommand::Metadata(input)) => metadata(api, input, false)?,
        Command::Storage(StorageCommand::Show { id }) => Remote::StorageShow(resource(id)),
        Command::Storage(StorageCommand::Test { id }) => Remote::StorageTest(resource(id)),
        Command::Storage(StorageCommand::Create { id, from }) => {
            return write::storage(api, id, from, false, false).await;
        }
        Command::Storage(StorageCommand::Replace { id, from, yes }) => {
            return write::storage(api, id, from, true, *yes).await;
        }
        Command::Storage(StorageCommand::Delete { id, yes }) => {
            write::confirm(api, *yes, "Delete", &format!("storage {id}"))?;
            Remote::StorageDelete(resource(id))
        }
        Command::Client(ClientCommand::List) => Remote::ClientList(EmptyInput {}),
        Command::Client(ClientCommand::Show { id }) => Remote::ClientShow(resource(id)),
        Command::Client(ClientCommand::Create { id, storage }) => {
            Remote::ClientCreate(ClientCreateInput {
                id: id.clone(),
                storage_id: storage.clone(),
            })
        }
        Command::Client(ClientCommand::Delete { id, yes }) => {
            write::confirm(api, *yes, "Delete", &format!("client {id}"))?;
            Remote::ClientDelete(resource(id))
        }
        Command::Credential(CredentialCommand::List { client }) => {
            Remote::CredentialList(client_input(client))
        }
        Command::Credential(CredentialCommand::Create { client, secret_out }) => {
            return write::credential_create(api, client, secret_out).await;
        }
        Command::Credential(CredentialCommand::Delete {
            client,
            access_key_id,
            yes,
        }) => {
            write::confirm(
                api,
                *yes,
                "Delete",
                &format!("credential {access_key_id} for client {client}"),
            )?;
            Remote::CredentialDelete(CredentialDeleteInput {
                client_id: client.clone(),
                access_key_id: access_key_id.clone(),
            })
        }
        Command::ClientKey(ClientKeyCommand::List { client }) => {
            Remote::ClientKeyList(client_input(client))
        }
        Command::ClientKey(ClientKeyCommand::Register { client, key_file }) => {
            Remote::ClientKeyRegister(ClientKeyInput {
                client_id: client.clone(),
                key_hash: crate::input::client_key_hash(key_file)?,
            })
        }
        Command::ClientKey(ClientKeyCommand::Delete {
            client,
            key_hash,
            yes,
        }) => {
            write::confirm(
                api,
                *yes,
                "Delete",
                &format!("client key {key_hash} for client {client}"),
            )?;
            Remote::ClientKeyDelete(ClientKeyInput {
                client_id: client.clone(),
                key_hash: key_hash.clone(),
            })
        }
        Command::Usage(Usage::Storages) => Remote::UsageStorages(EmptyInput {}),
        Command::Usage(Usage::Clients) => Remote::UsageClients(EmptyInput {}),
        Command::Usage(Usage::History { days }) => {
            Remote::UsageHistory(HistoryInput { days: *days })
        }
    };
    result::execute(api, command).await
}

fn resource(id: &str) -> ResourceInput {
    ResourceInput { id: id.into() }
}

fn metadata(api: &Api, input: &MetadataCommand, storage: bool) -> Result<Remote, Error> {
    Ok(match input {
        MetadataCommand::Show { id } => {
            if storage {
                Remote::StorageMetadataShow(resource(id))
            } else {
                Remote::ClientMetadataShow(resource(id))
            }
        }
        MetadataCommand::Replace { id, from, yes } => {
            let metadata = crate::input::metadata(from)?;
            write::confirm(
                api,
                *yes,
                "Replace metadata for",
                &format!("{} {id}", if storage { "storage" } else { "client" }),
            )?;
            let input = MetadataInput {
                id: id.clone(),
                metadata,
            };
            if storage {
                Remote::StorageMetadataReplace(input)
            } else {
                Remote::ClientMetadataReplace(input)
            }
        }
    })
}
fn client_input(id: &str) -> ClientInput {
    ClientInput {
        client_id: id.into(),
    }
}
