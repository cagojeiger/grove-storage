#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod support;
use serde_json::json;
use support::*;

#[test]
fn successful_envelope_requires_protocol_command_and_request_identity() {
    let valid = json!({"protocol":1,"request_id":REQUEST_ID,"command":"client.create","result":{"id":"app","storage_id":"local"}});
    for (field, value) in [
        ("protocol", json!(2)),
        ("command", json!("client.delete")),
        ("request_id", json!("invalid")),
        ("request_id", json!("00000000-0000-0000-0000-000000000000")),
        ("error", json!({"code":"conflict","outcome":"not_applied"})),
    ] {
        let mut reply = valid.clone();
        reply[field] = value;
        let server = Server::new(vec![("client.create", Reply::wire(reply))]);
        let result = envelope(
            &server.run(&["client", "create", "app", "--storage", "local"]),
            8,
        );
        assert_eq!(result["error"]["code"], "invalid_response");
        assert_eq!(result["error"]["outcome"], "unknown");
        assert_eq!(server.seen().len(), 1);
    }
}

#[test]
fn structured_failure_preserves_outcome_independently_of_http_status() {
    for (outcome, exit) in [("not_applied", 5), ("unknown", 8), ("applied", 8)] {
        let server = Server::new(vec![(
            "client.create",
            Reply::rejected(503, "unavailable", outcome),
        )]);
        let result = envelope(
            &server.run(&["client", "create", "app", "--storage", "local"]),
            exit,
        );
        assert_eq!(result["error"]["code"], "unavailable");
        assert_eq!(result["error"]["outcome"], outcome);
        assert_eq!(server.seen().len(), 1);
    }
    for reply in [
        Reply::error(401),
        Reply::error(404),
        Reply::rejected(503, "conflict", "not_applied"),
    ] {
        let server = Server::new(vec![("client.create", reply)]);
        let result = envelope(
            &server.run(&["client", "create", "app", "--storage", "local"]),
            8,
        );
        assert_eq!(result["error"]["outcome"], "unknown");
    }
}

#[test]
fn admission_rejection_is_not_applied_and_never_retries_a_mutation() {
    let server = Server::new(vec![(
        "client.create",
        Reply::rejected(429, "rate_limited", "not_applied"),
    )]);
    let result = envelope(
        &server.run(&["client", "create", "app", "--storage", "local"]),
        7,
    );
    assert_eq!(result["error"]["code"], "rate_limited");
    assert_eq!(result["error"]["outcome"], "not_applied");
    assert_eq!(server.seen().len(), 1);

    let server = Server::new(vec![(
        "client.create",
        Reply::status(429, json!({"error":"rate_limited"})),
    )]);
    let result = envelope(
        &server.run(&["client", "create", "app", "--storage", "local"]),
        8,
    );
    assert_eq!(result["error"]["outcome"], "unknown");
    assert_eq!(server.seen().len(), 1);
}

#[test]
fn rate_limited_issuance_discards_the_secret_marker() {
    let server = Server::new(vec![(
        "credential.create",
        Reply::rejected(429, "rate_limited", "not_applied"),
    )]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("secret.json");
    let output = server
        .command()
        .args(["credential", "create", "--client", "app", "--secret-out"])
        .arg(&path)
        .output()
        .unwrap();
    let result = envelope(&output, 7);
    assert_eq!(result["error"]["outcome"], "not_applied");
    assert!(!path.exists());
}

#[test]
fn delete_result_is_correlated_to_resource_client_and_id() {
    for reply in [
        json!({"resource":"storage","id":"app","client_id":null}),
        json!({"resource":"client","id":"other","client_id":null}),
        json!({"resource":"client","id":"app","client_id":"other"}),
    ] {
        let server = Server::new(vec![("client.delete", Reply::json(reply))]);
        let result = envelope(&server.run(&["client", "delete", "app", "--yes"]), 8);
        assert_eq!(result["error"]["outcome"], "unknown");
    }
}

#[test]
fn incompatible_server_does_not_trigger_legacy_fallback() {
    let server = Server::new(vec![(
        "client.list",
        Reply::rejected(400, "protocol_incompatible", "not_applied"),
    )]);
    let result = envelope(&server.run(&["client", "list"]), 7);
    assert_eq!(result["error"]["code"], "protocol_incompatible");
    assert_eq!(server.seen().len(), 1);
    assert_eq!(server.seen()[0].path, COMMAND_PATH);
}

#[test]
fn definitive_failed_issuance_discards_marker_even_for_503() {
    let server = Server::new(vec![(
        "credential.create",
        Reply::rejected(503, "unavailable", "not_applied"),
    )]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("secret.json");
    let output = server
        .command()
        .args(["credential", "create", "--client", "app", "--secret-out"])
        .arg(&path)
        .output()
        .unwrap();
    let result = envelope(&output, 5);
    assert_eq!(result["error"]["outcome"], "not_applied");
    assert!(!path.exists());
}
