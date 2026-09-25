use crate::error::Error;
use grove_management_command::{
    COMMAND_PROTOCOL_VERSION, Command, CommandError, Effect, Outcome, Output,
};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Success {
    protocol: u16,
    request_id: Uuid,
    command: String,
    result: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    protocol: u16,
    request_id: Uuid,
    error: CommandError,
}

pub(super) fn decode(command: &Command, status: u16, bytes: &[u8]) -> Result<Output, Error> {
    let name = command.name();
    let mutation = name.effect() == Effect::Mutation;
    if status != 200 {
        if let Ok(reply) = serde_json::from_slice::<Failure>(bytes)
            && reply.protocol == COMMAND_PROTOCOL_VERSION
            && !reply.request_id.is_nil()
            && (mutation || reply.error.outcome == Outcome::NotApplied)
            && Error::command_status(reply.error.code) == status
        {
            return Err(Error::command(reply.error, status));
        }
        return Err(Error::untrusted_response(status, mutation));
    }
    let invalid = || Error::unverified_result(status, mutation);
    let reply: Success = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if reply.protocol != COMMAND_PROTOCOL_VERSION
        || reply.request_id.is_nil()
        || reply.command != name.as_str()
    {
        return Err(invalid());
    }
    let output = name.decode_output(reply.result).map_err(|_| invalid())?;
    if !corresponds(command, &output) {
        return Err(invalid());
    }
    Ok(output)
}

fn corresponds(command: &Command, output: &Output) -> bool {
    match (command, output) {
        (Command::StorageShow(i), Output::StorageShow(o)) => i.id == o.id,
        (Command::StorageTest(i), Output::StorageTest(o)) => {
            i.id == o.id && o.state == grove_management_command::model::State::Ok
        }
        (Command::StorageCreate(i), Output::StorageCreate(o))
        | (Command::StorageReplace(i), Output::StorageReplace(o)) => i.id == o.id,
        (Command::ClientShow(i), Output::ClientShow(o)) => i.id == o.id,
        (Command::ClientCreate(i), Output::ClientCreate(o)) => {
            i.id == o.id && i.storage_id == o.storage_id
        }
        (Command::ClientKeyRegister(i), Output::ClientKeyRegister(o)) => {
            i.client_id == o.client_id && i.key_hash == o.key_hash
        }
        (Command::StorageDelete(i), Output::StorageDelete(o)) => {
            o.resource == "storage" && i.id == o.id && o.client_id.is_none()
        }
        (Command::ClientDelete(i), Output::ClientDelete(o)) => {
            o.resource == "client" && i.id == o.id && o.client_id.is_none()
        }
        (Command::ClientKeyDelete(i), Output::ClientKeyDelete(o)) => {
            o.resource == "client-key"
                && i.key_hash == o.id
                && o.client_id.as_deref() == Some(&i.client_id)
        }
        (Command::CredentialDelete(i), Output::CredentialDelete(o)) => {
            o.resource == "credential"
                && i.access_key_id == o.id
                && o.client_id.as_deref() == Some(&i.client_id)
        }
        _ => true,
    }
}
