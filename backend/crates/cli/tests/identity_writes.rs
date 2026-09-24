#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod support;

use serde_json::{Value, json};
use support::*;

#[test]
fn client_and_key_lifecycle_hashes_raw_key_locally() {
    let hash = "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let key_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(key_path.path(), "abc\n").unwrap();
    let server = Server::routes(vec![
        (
            "POST",
            "client.create",
            Reply::status(200, json!({"id":"app","storage_id":"archive"})),
        ),
        (
            "POST",
            "client-key.register",
            Reply::status(200, json!({"client_id":"app","key_hash":hash})),
        ),
        (
            "POST",
            "client-key.delete",
            Reply::json(json!({"resource":"client-key","id":hash,"client_id":"app"})),
        ),
        (
            "POST",
            "credential.delete",
            Reply::json(json!({"resource":"credential","id":"fgakpublic","client_id":"app"})),
        ),
        (
            "POST",
            "client.delete",
            Reply::json(json!({"resource":"client","id":"app","client_id":null})),
        ),
    ]);

    envelope(
        &server.run(&["client", "create", "app", "--storage", "archive"]),
        0,
    );
    let registered = server
        .command()
        .args(["client-key", "register", "--client", "app", "--key-file"])
        .arg(key_path.path())
        .output()
        .unwrap();
    let result = envelope(&registered, 0);
    assert_eq!(result["data"]["key_hash"], hash);
    assert!(!String::from_utf8_lossy(&registered.stdout).contains("abc"));

    envelope(
        &server.run(&["client-key", "delete", "--client", "app", hash, "--yes"]),
        0,
    );
    envelope(
        &server.run(&[
            "credential",
            "delete",
            "--client",
            "app",
            "fgakpublic",
            "--yes",
        ]),
        0,
    );
    envelope(&server.run(&["client", "delete", "app", "--yes"]), 0);

    let seen = server.seen();
    let client_body: Value = serde_json::from_str(&seen[0].body).unwrap();
    let key_body: Value = serde_json::from_str(&seen[1].body).unwrap();
    assert_eq!(
        client_body["input"],
        json!({"id":"app","storage_id":"archive"})
    );
    assert_eq!(
        key_body["input"],
        json!({"client_id":"app","key_hash":hash})
    );
    assert!(!seen[1].body.contains("abc"));
    assert_eq!(
        seen.iter()
            .filter(|request| request.method == "POST")
            .count(),
        5
    );
}
