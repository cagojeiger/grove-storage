use super::CommandResult;
use crate::{
    error::Error,
    http::Api,
    model::{self, Data},
};
use grove_management_command::{
    Command, Output,
    input::{ClientInput, StorageInput},
};
use std::path::Path;

pub async fn storage(api: &Api, id: &str, path: &Path, replace: bool, yes: bool) -> CommandResult {
    let spec = crate::input::storage_spec(path)?;
    if replace {
        confirm(api, yes, "Replace", &format!("storage {id}"))?;
    }
    let input = StorageInput {
        id: id.into(),
        spec,
    };
    super::result::execute(
        api,
        if replace {
            Command::StorageReplace(input)
        } else {
            Command::StorageCreate(input)
        },
    )
    .await
}

pub async fn credential_create(api: &Api, client: &str, path: &Path) -> CommandResult {
    let mut output = crate::secret::SecretOutput::create(path)?;
    let issued = match api
        .execute(Command::CredentialCreate(ClientInput {
            client_id: client.into(),
        }))
        .await
    {
        Ok(Output::CredentialCreate(issued)) => issued,
        Ok(_) => {
            return Ok((
                credential_delivery(client, None, &output, output.state()),
                Some(Error::unverified_result(200, true)),
            ));
        }
        Err(error) if error.outcome == "not_applied" => {
            output.discard();
            return Err(error);
        }
        Err(error) => {
            return Ok((
                credential_delivery(client, None, &output, output.state()),
                Some(error),
            ));
        }
    };
    let reported_access_key_id = crate::args::valid_access_key_id(&issued.access_key_id)
        .then(|| issued.access_key_id.clone());
    if reported_access_key_id.is_none() || issued.secret_key.is_empty() {
        return Ok((
            credential_delivery(client, reported_access_key_id, &output, output.state()),
            Some(Error::applied_invalid_response(200)),
        ));
    }
    let access_key_id = issued.access_key_id;
    if output
        .write_credential(client, &access_key_id, &issued.secret_key)
        .is_err()
    {
        return Ok((
            credential_delivery(client, Some(access_key_id), &output, output.state()),
            Some(Error::applied_secret_write()),
        ));
    }
    Ok((
        credential_delivery(client, Some(access_key_id), &output, "saved"),
        None,
    ))
}

pub(super) fn confirm(api: &Api, yes: bool, action: &str, resource: &str) -> Result<(), Error> {
    crate::confirm::destructive(yes, &api.origin(), action, resource)
}
fn credential_delivery(
    client: &str,
    access_key_id: Option<String>,
    output: &crate::secret::SecretOutput,
    state: &'static str,
) -> Data {
    Data::CredentialDelivery(model::CredentialDelivery {
        client_id: client.into(),
        access_key_id,
        secret_file: output.path().to_owned(),
        file_state: state,
    })
}
