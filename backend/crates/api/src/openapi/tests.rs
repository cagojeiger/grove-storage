#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use tower::ServiceExt;

fn check_references(root: &Value, value: &Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get("$ref") {
                let pointer = reference.strip_prefix('#').unwrap();
                assert!(root.pointer(pointer).is_some(), "unresolved {reference}");
            }
            for child in object.values() {
                check_references(root, child);
            }
        }
        Value::Array(array) => {
            for child in array {
                check_references(root, child);
            }
        }
        _ => {}
    }
}

#[test]
fn all_documents_have_resolvable_local_references() {
    for document in documents().as_object().unwrap().values() {
        assert_eq!(document["openapi"], "3.1.0");
        check_references(document, document);
    }
}

#[test]
fn catalog_covers_each_command_and_correlates_results() {
    let doc = management();
    let inputs = doc["components"]["schemas"]["CommandRequest"]["oneOf"]
        .as_array()
        .unwrap();
    let outputs = doc["components"]["schemas"]["CommandResult"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(inputs.len(), CommandName::ALL.len());
    assert_eq!(outputs.len(), CommandName::ALL.len());
    for ((input, output), command) in inputs.iter().zip(outputs).zip(CommandName::ALL) {
        assert_eq!(input["properties"]["command"]["const"], command.as_str());
        assert_eq!(output["properties"]["command"]["const"], command.as_str());
        assert_eq!(
            input["properties"]["protocol"]["const"],
            COMMAND_PROTOCOL_VERSION
        );
        assert_eq!(input["additionalProperties"], false);
    }
    assert!(doc["paths"].get("/api/admin/console-commands/v1").is_none());
}

#[test]
fn native_types_and_s3_auth_remain_distinct() {
    let doc = native();
    assert_eq!(
        doc["components"]["schemas"]["CreateOut"]["properties"]["file_id"]["format"],
        "uuid"
    );
    assert_eq!(
        doc["paths"]["/api/v1/files/{id}"]["delete"]["responses"]["200"]["description"],
        "Success"
    );
    assert_eq!(
        doc["paths"]["/api/v1/files/{id}/read"]["post"]["requestBody"]["required"],
        false
    );
    let s3 = s3();
    assert!(
        s3["components"]["securitySchemes"]
            .get("AccountToken")
            .is_none()
    );
    assert_eq!(
        s3["components"]["securitySchemes"]["SigV4Query"]["name"],
        "X-Amz-Signature"
    );
    assert_eq!(s3["paths"]["/{bucket}/{key}"].as_object().unwrap().len(), 6);
}

#[tokio::test]
async fn documentation_is_public_without_database_access_and_does_not_bypass_auth() {
    let mut state = crate::routes::tests::test_state();
    state.pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(10))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let app = crate::routes::app(state, &[]);
    for surface in ["management", "native", "s3"] {
        let request = Request::builder()
            .uri(format!("/api/docs/{surface}.json"))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let doc: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(doc, documents()[surface]);
    }
    for path in ["/api/v1/files", "/api/admin/commands/v1"] {
        let request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"protocol":1,"command":"status","input":{}}"#,
            ))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        // Native rejects a missing key immediately. Management rechecks identity
        // transactionally and fails closed when its database is unavailable.
        let expected = if path.starts_with("/api/v1") {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };
        assert_eq!(response.status(), expected);
    }
    let request = Request::builder()
        .uri("/api/docs/identity.json")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
}
