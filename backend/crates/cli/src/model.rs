use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use grove_management_command::model::{
    Client, ClientKey, ClientUsage, Deleted, IssuedCredential, Snapshot, Storage, StorageUsage,
};

#[derive(Serialize)]
pub struct CredentialDelivery {
    pub client_id: String,
    pub access_key_id: Option<String>,
    pub secret_file: PathBuf,
    pub file_state: &'static str,
}

#[derive(Deserialize)]
pub struct Identity {
    pub name: String,
    pub version: String,
}

#[derive(Deserialize)]
pub struct Health {
    pub status: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum Data {
    Update(crate::update::UpdateResult),
    Installation { path: std::path::PathBuf },
    Strings(Vec<String>),
    Storages(Vec<Storage>),
    Storage(Storage),
    Client(Client),
    ClientKey(ClientKey),
    CredentialDelivery(CredentialDelivery),
    Deleted(Deleted),
    StorageUsage(Vec<StorageUsage>),
    ClientUsage(Vec<ClientUsage>),
    History(Vec<Snapshot>),
    Status(crate::status::Status),
}
