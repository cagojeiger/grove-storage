use super::CommandResult;
use crate::{error::Error, http::Api, model::Data};
use grove_management_command::{Command, Output};

pub(super) async fn execute(api: &Api, command: Command) -> CommandResult {
    let data = match api.execute(command).await? {
        Output::Status(row) => {
            use grove_management_command::model::State;
            let healthy = [
                row.identity,
                row.health,
                row.readiness,
                row.registry.state,
                row.registry.usage,
                row.registry.clients,
            ]
            .into_iter()
            .all(|state| state == State::Ok);
            return Ok((
                Data::Status(row),
                (!healthy).then(|| {
                    Error::new(
                        "status_failed",
                        "The server reports a failed or unknown status",
                        5,
                    )
                }),
            ));
        }
        Output::StorageList(mut rows) => {
            rows.sort_by(|a, b| a.id.cmp(&b.id));
            Data::Storages(rows)
        }
        Output::StorageShow(row) | Output::StorageCreate(row) | Output::StorageReplace(row) => {
            Data::Storage(row)
        }
        Output::ClientList(mut rows)
        | Output::ClientKeyList(mut rows)
        | Output::CredentialList(mut rows) => {
            rows.sort();
            Data::Strings(rows)
        }
        Output::ClientShow(row) | Output::ClientCreate(row) => Data::Client(row),
        Output::ClientKeyRegister(row) => Data::ClientKey(row),
        Output::StorageDelete(row)
        | Output::ClientDelete(row)
        | Output::CredentialDelete(row)
        | Output::ClientKeyDelete(row) => Data::Deleted(row),
        Output::UsageStorages(mut rows) => {
            rows.sort_by(|a, b| a.storage_id.cmp(&b.storage_id));
            Data::StorageUsage(rows)
        }
        Output::UsageClients(mut rows) => {
            rows.sort_by(|a, b| (&a.client_id, &a.storage_id).cmp(&(&b.client_id, &b.storage_id)));
            Data::ClientUsage(rows)
        }
        Output::UsageHistory(mut rows) => {
            rows.sort_by(|a, b| {
                (&a.day, &a.storage_id, &a.client_id).cmp(&(&b.day, &b.storage_id, &b.client_id))
            });
            Data::History(rows)
        }
        // One-time secrets are delivered only through the reserved private file.
        Output::CredentialCreate(_) => return Err(Error::applied_invalid_response(200)),
    };
    Ok((data, None))
}
