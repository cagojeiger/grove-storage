#![allow(dead_code)]

use grove_management_command::CommandName;
use serde_json::{Value, json};

pub const SECRET: &str = "sentinel-never-in-debug-or-error";

pub fn hash() -> String {
    format!("sha256:{}", "a".repeat(64))
}

pub fn input(name: CommandName) -> Value {
    match name {
        CommandName::Status
        | CommandName::StorageList
        | CommandName::ClientList
        | CommandName::UsageStorages
        | CommandName::UsageClients => json!({}),
        CommandName::StorageShow
        | CommandName::StorageMetadataShow
        | CommandName::ClientMetadataShow
        | CommandName::StorageTest
        | CommandName::StorageDelete
        | CommandName::ClientShow
        | CommandName::ClientDelete => json!({"id":"app"}),
        CommandName::StorageCreate | CommandName::StorageReplace => json!({
            "id":"primary", "spec": {"kind":"s3", "endpoint":"https://s3.example.com",
                "region":"test", "bucket":"data", "access_key":"access", "secret_key":SECRET,
                "capacity_bytes":42}
        }),
        CommandName::ClientCreate => json!({"id":"app", "storage_id":"primary"}),
        CommandName::StorageMetadataReplace | CommandName::ClientMetadataReplace => {
            json!({"id":"app", "metadata":{"description":SECRET}})
        }
        CommandName::CredentialList
        | CommandName::CredentialCreate
        | CommandName::ClientKeyList => json!({"client_id":"app"}),
        CommandName::CredentialDelete => json!({"client_id":"app", "access_key_id":"fgak12345678"}),
        CommandName::ClientKeyRegister | CommandName::ClientKeyDelete => {
            json!({"client_id":"app", "key_hash":hash()})
        }
        CommandName::UsageHistory => json!({"days":90}),
    }
}

pub fn storage() -> Value {
    json!({"id":"primary", "kind":"s3", "force_relay":false, "root_path":null,
        "endpoint":"https://s3.example.com", "public_endpoint":"https://cdn.example.com",
        "region":"test", "bucket":"data", "force_path_style":false, "access_key":"access",
        "capacity_bytes":42})
}

pub fn output(name: CommandName) -> Value {
    match name {
        CommandName::StorageMetadataShow
        | CommandName::ClientMetadataShow
        | CommandName::StorageMetadataReplace
        | CommandName::ClientMetadataReplace => json!({"id":"app", "metadata":{}}),
        CommandName::Status => json!({"server_version":"0.4.1", "identity":"ok", "health":"ok",
            "readiness":"ok", "registry":{"state":"ok", "usage":"ok", "clients":"ok",
                "storage_count":1, "client_count":1}, "storage_access":"not_checked"}),
        CommandName::StorageList => json!([storage()]),
        CommandName::StorageTest => json!({"id":"primary", "state":"ok"}),
        CommandName::StorageShow | CommandName::StorageCreate | CommandName::StorageReplace => {
            storage()
        }
        CommandName::ClientList => json!(["app"]),
        CommandName::ClientShow | CommandName::ClientCreate => {
            json!({"id":"app", "storage_id":"primary"})
        }
        CommandName::CredentialList => json!(["fgak12345678"]),
        CommandName::CredentialCreate => {
            json!({"access_key_id":"fgak12345678", "secret_key":SECRET})
        }
        CommandName::ClientKeyList => json!([hash()]),
        CommandName::ClientKeyRegister => json!({"client_id":"app", "key_hash":hash()}),
        CommandName::StorageDelete => json!({"resource":"storage", "id":"primary"}),
        CommandName::ClientDelete => json!({"resource":"client", "id":"app"}),
        CommandName::CredentialDelete => {
            json!({"resource":"credential", "client_id":"app", "id":"fgak12345678"})
        }
        CommandName::ClientKeyDelete => {
            json!({"resource":"client-key", "client_id":"app", "id":hash()})
        }
        CommandName::UsageStorages => {
            json!([{"storage_id":"primary", "kind":"s3", "capacity_bytes":42,
            "reserved_bytes":1, "active_bytes":40, "purge_pending_bytes":2, "remaining_bytes":-1,
            "reserved_files":1, "active_files":4, "purge_pending_files":2}])
        }
        CommandName::UsageClients => {
            json!([{"client_id":"app", "storage_id":"primary", "active_files":4, "active_bytes":40}])
        }
        CommandName::UsageHistory => {
            json!([{"day":"2026-09-24", "storage_id":"primary", "client_id":"app", "active_bytes":40, "active_files":4}])
        }
    }
}
