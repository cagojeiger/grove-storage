#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod support;

use serde_json::json;
use support::*;

#[test]
fn every_read_command_uses_common_contract_and_no_db_configuration() {
    let cases = [
        (
            vec!["storage", "test", "r2"],
            "storage.test",
            json!({"id":"r2", "state":"ok"}),
            "storage.test",
        ),
        (
            vec!["storage", "list"],
            "storage.list",
            json!([storage()]),
            "storage.list",
        ),
        (
            vec!["storage", "show", "r2"],
            "storage.show",
            storage(),
            "storage.show",
        ),
        (
            vec!["client", "list"],
            "client.list",
            json!(["z", "a"]),
            "client.list",
        ),
        (
            vec!["client", "show", "app"],
            "client.show",
            json!({"id":"app","storage_id":"r2"}),
            "client.show",
        ),
        (
            vec!["credential", "list", "--client", "app"],
            "credential.list",
            json!(["ZKEY", "AKEY"]),
            "credential.list",
        ),
        (
            vec!["client-key", "list", "--client", "app"],
            "client-key.list",
            json!(["zhash", "ahash"]),
            "client-key.list",
        ),
        (
            vec!["usage", "storages"],
            "usage.storages",
            json!([usage()]),
            "usage.storages",
        ),
        (
            vec!["usage", "clients"],
            "usage.clients",
            json!([{"client_id":"app","storage_id":"r2","active_files":2,"active_bytes":10}]),
            "usage.clients",
        ),
        (
            vec!["usage", "history"],
            "usage.history",
            json!([{"day":"2026-09-08","storage_id":"r2","client_id":"app","active_files":2,"active_bytes":10}]),
            "usage.history",
        ),
    ];
    for (args, path, data, name) in cases {
        let server = Server::new(vec![(path, Reply::json(data))]);
        let output = server.run(&args);
        let result = envelope(&output, 0);
        assert_eq!(result["schema_version"], 1);
        assert_eq!(result["command"], name);
        assert_eq!(result["ok"], true);
        assert_eq!(result["endpoint"], server.endpoint);
        assert!(result["error"].is_null());
        assert!(output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("DO-NOT-PRINT"));
        let seen = server.seen();
        assert_eq!(seen.len(), 1, "no implicit N+1 requests");
        assert_eq!(seen[0].path, COMMAND_PATH);
        assert_eq!(seen[0].method, "POST");
        let body: serde_json::Value = serde_json::from_str(&seen[0].body).unwrap();
        assert_eq!(body["protocol"], 1);
        assert_eq!(body["command"], name);
        assert_eq!(
            seen[0].authorization.as_deref(),
            Some(format!("Bearer {TOKEN}").as_str())
        );
    }
}

#[test]
fn history_days_and_resource_ids_are_preserved_in_json() {
    let server = Server::new(vec![
        ("usage.history", Reply::json(json!([]))),
        ("credential.list", Reply::json(json!([]))),
    ]);
    envelope(&server.run(&["usage", "history", "--days", "7"]), 0);
    envelope(
        &server.run(&["credential", "list", "--client", "a/b?#%"]),
        0,
    );
    assert_eq!(server.seen().len(), 2);
    let seen = server.seen();
    let history: serde_json::Value = serde_json::from_str(&seen[0].body).unwrap();
    let client: serde_json::Value = serde_json::from_str(&seen[1].body).unwrap();
    assert_eq!(history["input"]["days"], 7);
    assert_eq!(client["input"]["client_id"], "a/b?#%");
}

#[test]
fn output_sorts_lists_and_preserves_signed_integer_bytes_exactly() {
    let server = Server::new(vec![
        ("client.list", Reply::json(json!(["z", "a"]))),
        ("usage.storages", Reply::json(json!([usage()]))),
    ]);
    let result = envelope(&server.run(&["client", "list"]), 0);
    assert_eq!(result["data"], json!(["a", "z"]));
    let result = envelope(&server.run(&["usage", "storages"]), 0);
    assert_eq!(
        result["data"][0]["active_bytes"].as_i64(),
        Some(9007199254740993)
    );
    assert_eq!(
        result["data"][0]["remaining_bytes"].as_i64(),
        Some(-9007199254740993)
    );
    assert_eq!(result["data"][0]["capacity_bytes"], 0);
}

#[test]
fn show_rejects_a_mismatched_identity() {
    let server = Server::new(vec![(
        "client.show",
        Reply::json(json!({"id":"other","storage_id":"r2"})),
    )]);
    let result = envelope(&server.run(&["client", "show", "app"]), 5);
    assert_eq!(result["error"]["code"], "invalid_response");
}

#[test]
fn storage_test_rejects_wrong_target_and_non_success_without_retry() {
    for result in [
        json!({"id":"other","state":"ok"}),
        json!({"id":"r2","state":"failed"}),
    ] {
        let server = Server::new(vec![("storage.test", Reply::json(result))]);
        let result = envelope(&server.run(&["storage", "test", "r2"]), 5);
        assert_eq!(result["error"]["code"], "invalid_response");
        assert_eq!(server.seen().len(), 1);
    }
}

#[test]
fn storage_table_excludes_unknown_secret_fields() {
    let server = Server::new(vec![("storage.show", Reply::json(storage()))]);
    let output = server
        .command()
        .args(["storage", "show", "r2", "--output", "table"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("0 B"));
    assert!(text.contains("public-access-key"));
    assert!(!text.contains("DO-NOT-PRINT"));
    assert!(!text.contains("secret_key"));
}
