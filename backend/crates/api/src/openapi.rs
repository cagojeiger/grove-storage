//! Public integration contracts only; browser identity and legacy admin are internal/retired.

// Only fixed, locally constructed JSON objects are indexed here, never request data.
#![allow(clippy::indexing_slicing)]

use axum::{Json, Router, http::header, response::IntoResponse, routing::get};
use grove_management_command::{COMMAND_PROTOCOL_VERSION, CommandError, CommandName};
use serde_json::{Map, Value, json};

use crate::routes::AppState;

pub(crate) fn documents() -> Value {
    json!({"management": management(), "native": native(), "s3": s3()})
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/docs/management.json",
            get(|| async { reply(management()) }),
        )
        .route("/api/docs/native.json", get(|| async { reply(native()) }))
        .route("/api/docs/s3.json", get(|| async { reply(s3()) }))
}

fn reply(document: Value) -> impl IntoResponse {
    (
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        Json(document),
    )
}

fn document(title: &str, description: &str) -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {"title": title, "version": env!("CARGO_PKG_VERSION"), "description": description},
        "servers": [{"url": "/"}],
        "paths": {},
        "components": {"schemas": {}, "securitySchemes": {}}
    })
}

fn reference(name: &str) -> Value {
    json!({"$ref": format!("#/components/schemas/{name}")})
}

// Catalog schemas are independent documents. Keep each definition namespace
// isolated when embedding them, including nested and recursive references.
fn embed(schema: schemars::Schema, name: &str) -> Value {
    fn relocate(value: &mut Value, name: &str) {
        match value {
            Value::Object(object) => {
                if let Some(Value::String(reference)) = object.get_mut("$ref")
                    && let Some(pointer) = reference.strip_prefix('#')
                {
                    *reference = format!("#/components/schemas/{name}{pointer}");
                }
                for child in object.values_mut() {
                    relocate(child, name);
                }
            }
            Value::Array(array) => {
                for child in array {
                    relocate(child, name);
                }
            }
            _ => {}
        }
    }
    let mut value = schema.to_value();
    relocate(&mut value, name);
    value
}

fn body(schema: Value) -> Value {
    json!({"required": true, "content": {"application/json": {"schema": schema}}})
}

fn response(description: &str, schema: Value) -> Value {
    json!({"description": description, "content": {"application/json": {"schema": schema}}})
}

fn bearer(description: &str) -> Value {
    json!({"type": "http", "scheme": "bearer", "description": description})
}

fn management() -> Value {
    let mut doc = document(
        "Grove Storage - Management API",
        "Machine API for registry configuration and usage. Uses Account API tokens (gsm_...), never Client S3 keys or console cookies. Browser identity, console commands, legacy admin and MCP are not REST operations in this document. MCP uses the same command catalog via /api/admin/mcp. The Swagger view is read-only.",
    );
    let mut schemas = Map::new();
    let mut requests = Vec::new();
    let mut results = Vec::new();
    for command in CommandName::ALL {
        let name = command.as_str();
        let input = format!("{name}.input");
        let output = format!("{name}.output");
        schemas.insert(input.clone(), embed(command.input_schema(), &input));
        schemas.insert(output.clone(), embed(command.output_schema(), &output));
        requests.push(json!({
            "title": name, "type": "object", "additionalProperties": false,
            "description": format!("Permission: {:?}; effect: {:?}", command.required_action(), command.effect()),
            "required": ["protocol", "command", "input"],
            "properties": {"protocol": {"const": COMMAND_PROTOCOL_VERSION}, "command": {"const": name}, "input": reference(&input)}
        }));
        results.push(json!({
            "title": name, "type": "object", "required": ["protocol", "request_id", "command", "result"],
            "properties": {"protocol": {"const": COMMAND_PROTOCOL_VERSION}, "request_id": {"type": "string"}, "command": {"const": name}, "result": reference(&output)}
        }));
    }
    schemas.insert("CommandRequest".into(), json!({"oneOf": requests}));
    schemas.insert("CommandResult".into(), json!({"oneOf": results}));
    schemas.insert(
        "CommandError".into(),
        embed(schemars::schema_for!(CommandError), "CommandError"),
    );
    let error = json!({"type": "object", "required": ["protocol", "request_id", "error"],
        "properties": {"protocol": {"const": COMMAND_PROTOCOL_VERSION}, "request_id": {"type": "string"}, "error": reference("CommandError")}});
    let mut responses = Map::from_iter([(
        "200".into(),
        response(
            "Typed command result; credential secrets are returned only on issuance",
            reference("CommandResult"),
        ),
    )]);
    for (status, description) in [
        ("400", "Invalid envelope, protocol, command or input"),
        ("401", "Account token required; cookies are rejected"),
        ("403", "Role does not permit this command"),
        ("404", "Resource not found"),
        ("409", "Conflict"),
        (
            "429",
            "Request limit reached before command execution; retry after the response delay",
        ),
        ("500", "Internal failure"),
        ("503", "Dependency unavailable"),
    ] {
        responses.insert(status.into(), response(description, error.clone()));
    }
    doc["paths"] = json!({"/api/admin/commands/v1": {"post": {
        "operationId": "executeCommand", "summary": "Execute a management command", "tags": ["Management"],
        "requestBody": body(reference("CommandRequest")), "responses": responses,
        "security": [{"AccountToken": []}]
    }}});
    doc["components"]["schemas"] = Value::Object(schemas);
    doc["components"]["securitySchemes"] = json!({"AccountToken": bearer("Account API token. Cookie authentication is explicitly rejected on this endpoint.")});
    doc
}

fn native() -> Value {
    let mut doc = document(
        "Grove Storage - Native compatibility API",
        "FileGate-compatible JSON file lifecycle API. New S3 SDK integrations should use the S3 protocol instead. Authenticate using a Native Client key, not an Account token. Transfer bytes using the opaque, expiring URL returned by the API; then commit an upload. URL structure and provider identity are not a client contract. The Swagger view is read-only.",
    );
    let mut schemas: Map<String, Value> = crate::native::schemas()
        .map(|(name, schema)| (name.into(), embed(schema, name)))
        .collect();
    schemas.insert("NativeError".into(), json!({"type": "object", "required": ["error"], "properties": {"error": {"type": "string"}}}));
    let mut paths = Map::new();
    for (path, method, id, input, output, status, summary) in [
        (
            "/api/v1/files",
            "post",
            "createFile",
            Some("CreateBody"),
            "CreateOut",
            "201",
            "Create upload and issue PUT URL or multipart descriptor",
        ),
        (
            "/api/v1/files/{id}",
            "get",
            "statFile",
            None,
            "StatOut",
            "200",
            "Read file state and declared size",
        ),
        (
            "/api/v1/files/{id}",
            "delete",
            "deleteFile",
            None,
            "DeleteOut",
            "200",
            "Mark file deleted; physical cleanup is asynchronous",
        ),
        (
            "/api/v1/files/{id}/commit",
            "post",
            "commitFile",
            None,
            "CommitOut",
            "200",
            "Verify and commit single or multipart upload",
        ),
        (
            "/api/v1/files/{id}/parts",
            "post",
            "issuePartUrls",
            Some("PartsBody"),
            "PartsOut",
            "200",
            "Issue or renew URLs for requested multipart parts",
        ),
        (
            "/api/v1/files/{id}/read",
            "post",
            "issueReadUrl",
            Some("ReadBody"),
            "ReadOut",
            "200",
            "Issue GET URL for an active file",
        ),
    ] {
        let mut responses =
            Map::from_iter([(status.into(), response("Success", reference(output)))]);
        for (status, description) in [
            ("400", "Invalid request or upload verification failed"),
            ("401", "Native Client key required"),
            ("404", "File not found for this Client"),
            ("409", "File state conflict"),
            ("500", "Internal failure"),
            ("502", "Storage unavailable"),
            ("503", "Authentication database unavailable"),
        ] {
            responses.insert(
                status.into(),
                response(description, reference("NativeError")),
            );
        }
        let mut operation = json!({"operationId": id, "summary": summary, "tags": ["Files"], "responses": responses, "security": [{"ClientKey": []}]});
        if path.contains("{id}") {
            operation["parameters"] = json!([{"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}]);
        }
        if let Some(input) = input {
            operation["requestBody"] = body(reference(input));
            if input == "ReadBody" {
                operation["requestBody"]["required"] = json!(false);
            }
        }
        paths.entry(path).or_insert_with(|| json!({}))[method] = operation;
    }
    doc["paths"] = Value::Object(paths);
    doc["components"]["schemas"] = Value::Object(schemas);
    doc["components"]["securitySchemes"] = json!({"ClientKey": bearer("Native Client bearer key from the client-key registry. Account tokens and S3 credentials are separate.")});
    doc
}

fn s3() -> Value {
    let mut doc = document(
        "Grove Storage - S3-compatible API",
        "Path-style S3 protocol for SDKs and presigned upload/download. Bucket equals Client ID. Both header-signed and query-signed AWS SigV4 are supported. Account authentication is not used. Generate signed requests with an S3 SDK, not Swagger Authorize. The Swagger view is read-only. Supported: PutObject, HeadObject, GetObject, DeleteObject, CreateMultipartUpload, UploadPart, CompleteMultipartUpload and AbortMultipartUpload. Unsupported: ListBuckets, HeadBucket, ListObjectsV2, CopyObject, ListParts and ListMultipartUploads. Object bytes may pass through Grove to the configured S3 provider; the Client does not need to know that provider.",
    );
    let mut path = json!({"parameters": [
        {"name": "bucket", "in": "path", "required": true, "description": "Registered Client ID", "schema": {"type": "string"}},
        {"name": "key", "in": "path", "required": true, "description": "Logical object key; may contain slashes", "schema": {"type": "string"}}
    ]});
    for (method, summary, description, responses) in [
        (
            "put",
            "PutObject / UploadPart",
            "PUT bytes. Use partNumber and uploadId together for UploadPart. Single PutObject is limited to 5 GiB. Successful responses include ETag.",
            json!({"200": {"description": "Object or part stored; ETag header"}, "412": {"description": "Conditional PutObject failed"}}),
        ),
        (
            "get",
            "GetObject",
            "GET object bytes; supports bytes=a-b and bytes=a- Range. GET with uploadId (ListParts) is not supported.",
            json!({"200": {"description": "Object bytes", "content": {"application/octet-stream": {"schema": {"type": "string", "format": "binary"}}}}, "206": {"description": "Partial object bytes"}, "416": {"description": "Invalid range"}}),
        ),
        (
            "head",
            "HeadObject",
            "Read object metadata headers without downloading bytes.",
            json!({"200": {"description": "Content-Length, Content-Type and ETag headers"}}),
        ),
        (
            "delete",
            "DeleteObject / AbortMultipartUpload",
            "Without uploadId, delete the logical object (idempotent). With uploadId, abort the multipart upload bound to this key.",
            json!({"204": {"description": "Deleted or aborted"}}),
        ),
        (
            "post",
            "CreateMultipartUpload / CompleteMultipartUpload",
            "POST ?uploads creates a multipart session and returns XML UploadId. POST ?uploadId=... with CompleteMultipartUpload XML completes it. Queries are part of the SigV4 signature; these are not JSON REST operations.",
            json!({"200": {"description": "InitiateMultipartUploadResult or CompleteMultipartUploadResult XML", "content": {"application/xml": {"schema": {"type": "string"}}}}}),
        ),
    ] {
        let mut responses = responses;
        for (status, description) in [
            ("400", "Invalid request, checksum or multipart XML"),
            ("403", "SigV4 authentication failed"),
            ("404", "NoSuchBucket, NoSuchKey or NoSuchUpload"),
            ("501", "Unsupported S3 operation or option"),
            ("503", "Temporarily unavailable; retry"),
        ] {
            responses[status] = json!({"description": description});
        }
        let mut operation = json!({"operationId": format!("s3_{method}"), "tags": ["S3 objects"], "summary": summary, "description": description, "responses": responses});
        if matches!(method, "put" | "post" | "delete") {
            operation["parameters"] =
                json!([{"name": "uploadId", "in": "query", "schema": {"type": "string"}}]);
        }
        if method == "put" {
            if let Some(params) = operation["parameters"].as_array_mut() {
                params.push(json!({"name": "partNumber", "in": "query", "schema": {"type": "integer", "minimum": 1, "maximum": 10000}}));
            }
            operation["requestBody"] = json!({"required": true, "content": {"application/octet-stream": {"schema": {"type": "string", "format": "binary"}}}});
        }
        if method == "post" {
            if let Some(params) = operation["parameters"].as_array_mut() {
                params
                    .push(json!({"name": "uploads", "in": "query", "schema": {"type": "string"}}));
            }
            operation["requestBody"] = json!({"required": false, "content": {"application/xml": {"schema": {"type": "string"}}}});
        }
        path[method] = operation;
    }
    doc["paths"] = json!({"/{bucket}/{key}": path});
    // OpenAPI has no SigV4 signer. Never misrepresent an S3 secret as a bearer token.
    doc["security"] = json!([{"SigV4Header": []}, {"SigV4Query": []}]);
    doc["components"]["securitySchemes"] = json!({
        "SigV4Header": {"type": "apiKey", "in": "header", "name": "Authorization", "description": "Complete AWS4-HMAC-SHA256 authorization generated by an SDK; also requires signed host, date and payload hash headers."},
        "SigV4Query": {"type": "apiKey", "in": "query", "name": "X-Amz-Signature", "description": "SDK-generated presigned URL, including Algorithm, Credential, Date, Expires, SignedHeaders and Signature. Never enter only a secret here."}
    });
    doc
}

#[cfg(test)]
mod tests;
