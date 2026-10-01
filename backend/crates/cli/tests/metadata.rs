#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod support;
use serde_json::{Value, json};
use support::*;

#[test]
fn metadata_commands_use_shared_http_contract() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("labels.json");
    std::fs::write(&file, r#"{"environment":"home","description":"notes"}"#).unwrap();
    let metadata = json!({"environment":"home","description":"notes"});
    for resource in ["storage", "client"] {
        let show = format!("{resource}.metadata.show");
        let replace = format!("{resource}.metadata.replace");
        let server = Server::new(vec![
            (&show, Reply::json(json!({"id":"app","metadata":metadata}))),
            (
                &replace,
                Reply::json(json!({"id":"app","metadata":metadata})),
            ),
        ]);
        assert_eq!(
            envelope(&server.run(&[resource, "metadata", "show", "app"]), 0)["data"]["metadata"],
            metadata
        );
        assert_eq!(
            envelope(
                &server.run(&[
                    resource,
                    "metadata",
                    "replace",
                    "app",
                    "--from",
                    file.to_str().unwrap(),
                    "--yes"
                ]),
                0
            )["data"]["metadata"],
            metadata
        );
        let seen = server.seen();
        assert_eq!(seen.len(), 2);
        let body: Value = serde_json::from_str(&seen[1].body).unwrap();
        assert_eq!(body["input"], json!({"id":"app","metadata":metadata}));
    }
}

#[test]
fn invalid_metadata_is_rejected_before_http_and_wrong_echo_is_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("labels.json");
    let server = Server::new(vec![(
        "client.metadata.replace",
        Reply::json(json!({"id":"other","metadata":{}})),
    )]);
    for input in ["[]", "{\"x\":1}", "{\"x\":{}}", "not-json"] {
        std::fs::write(&file, input).unwrap();
        let output = server.run(&[
            "client",
            "metadata",
            "replace",
            "app",
            "--from",
            file.to_str().unwrap(),
            "--yes",
        ]);
        assert!(!output.status.success());
    }
    assert!(server.seen().is_empty());
    std::fs::write(&file, "{}").unwrap();
    let output = server.run(&[
        "client",
        "metadata",
        "replace",
        "app",
        "--from",
        file.to_str().unwrap(),
        "--yes",
    ]);
    assert!(!output.status.success());
    assert_eq!(server.seen().len(), 1);
    assert!(String::from_utf8_lossy(&output.stdout).contains("unknown"));
}
