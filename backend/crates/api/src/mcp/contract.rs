use grove_management_command::{CommandName, Effect};
use rmcp::model::{Tool, ToolAnnotations};
use serde_json::json;
use std::sync::Arc;

pub(super) fn tool(name: CommandName) -> Tool {
    let input = name.input_schema().as_object().cloned().unwrap_or_default();
    let mut result = name
        .output_schema()
        .as_object()
        .cloned()
        .unwrap_or_default();
    // Schemars references root definitions even when the result is an array.
    let definitions = result.remove("$defs").unwrap_or_else(|| json!({}));
    let output = json!({
        "type":"object", "$defs": definitions,
        "required":["protocol","request_id","command","result"],
        "properties":{
            "protocol":{"const":1}, "request_id":{"type":"string","format":"uuid"},
            "command":{"const":name.as_str()}, "result":result,
        }, "additionalProperties":false,
    });
    let read = name.effect() == Effect::Read;
    Tool::new(name.as_str(), description(name), input)
        .with_raw_output_schema(Arc::new(output.as_object().cloned().unwrap_or_default()))
        .with_annotations(
            ToolAnnotations::new()
                .read_only(read)
                .destructive(!read)
                .idempotent(read)
                .open_world(matches!(
                    name,
                    CommandName::StorageCreate
                        | CommandName::StorageReplace
                        | CommandName::StorageTest
                )),
        )
}

fn description(name: CommandName) -> &'static str {
    match name {
        CommandName::StorageMetadataShow | CommandName::ClientMetadataShow => {
            "Read resource metadata: a string-valued JSON label object, separate from S3 object metadata."
        }
        CommandName::StorageMetadataReplace | CommandName::ClientMetadataReplace => {
            "Replace all resource metadata; {} clears it. String values only, at most 8 KiB normalized JSON. Visible to resource readers; do not store secrets. Does not change routing, credentials, or provider objects."
        }
        CommandName::Status => {
            "Inspect server identity, database, and registry status; physical storage is not probed."
        }
        CommandName::StorageList => "List registered storages without provider secrets.",
        CommandName::StorageShow => "Show one registered storage without provider secrets.",
        CommandName::StorageTest => {
            "Test saved S3 settings from the server using HeadBucket and ListMultipartUploads, with a 10-second probe timeout. Does not write objects or test upload/download or public URLs."
        }
        CommandName::StorageCreate => {
            "Register a storage after an access probe. Provider credentials are sensitive inputs."
        }
        CommandName::StorageReplace => {
            "Replace a storage specification after an access probe. Supply the complete specification, including provider credentials. Existing file references protect address changes."
        }
        CommandName::StorageDelete => {
            "Delete an unreferenced storage. Client and file references block deletion."
        }
        CommandName::ClientList => "List registered Client IDs.",
        CommandName::ClientShow => "Show one Client and its storage assignment.",
        CommandName::ClientCreate => "Register a Client assigned to an existing storage.",
        CommandName::ClientDelete => {
            "Delete a Client without files, including its service keys. File references block deletion."
        }
        CommandName::CredentialList => "List a Client's public S3 access key IDs, not secrets.",
        CommandName::CredentialCreate => {
            "Issue a Client S3 credential. Returns the secret once to this MCP client; protect the response and transcript. An unknown outcome requires listing and revoking before reissuing."
        }
        CommandName::CredentialDelete => "Revoke the specified Client S3 credential.",
        CommandName::ClientKeyList => "List a Client's registered Native key hashes.",
        CommandName::ClientKeyRegister => {
            "Register a Native service key hash. Hash the raw key locally with SHA-256; send sha256: plus 64 lowercase hex digits."
        }
        CommandName::ClientKeyDelete => {
            "Revoke the specified Native service key hash for this Client."
        }
        CommandName::UsageStorages => "Inspect registered storage usage totals.",
        CommandName::UsageClients => "Inspect Client usage grouped by storage.",
        CommandName::UsageHistory => "List daily usage snapshots for 1-3650 days; default 90.",
    }
}
