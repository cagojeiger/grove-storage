#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod support;
use serde_json::{Value, json};
use support::*;

fn status(count: u64) -> Value {
    json!({"server_version":"0.4.1","identity":"ok","health":"ok","readiness":"ok",
        "registry":{"state":"ok","usage":"ok","clients":"ok","storage_count":count,"client_count":count},"storage_access":"not_checked"})
}

#[test]
fn status_is_one_authenticated_common_command_including_empty_registry() {
    for count in [0, 1] {
        let server = Server::new(vec![("status", Reply::json(status(count)))]);
        let result = envelope(&server.run(&["status"]), 0);
        assert_eq!(result["data"], status(count));
        let seen = server.seen();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path, COMMAND_PATH);
        assert_eq!(seen[0].method, "POST");
        assert_eq!(
            seen[0].authorization.as_deref(),
            Some(format!("Bearer {TOKEN}").as_str())
        );
        let body: Value = serde_json::from_str(&seen[0].body).unwrap();
        assert_eq!(body, json!({"protocol":1,"command":"status","input":{}}));
    }
}

#[test]
fn status_table_shows_endpoint_version_and_physical_check_limit() {
    let server = Server::new(vec![("status", Reply::json(status(1)))]);
    let output = server
        .command()
        .args(["status", "--output", "table"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [&server.endpoint, "0.4.1", "not_checked"] {
        assert!(text.contains(expected));
    }
}

#[test]
fn status_failure_has_no_fabricated_partial_health_results() {
    for (reply, exit) in [
        (Reply::rejected(403, "forbidden", "not_applied"), 3),
        (Reply::json(json!({})), 5),
    ] {
        let server = Server::new(vec![("status", reply)]);
        let result = envelope(&server.run(&["status"]), exit);
        assert!(result["data"].is_null());
        assert_eq!(server.seen().len(), 1);
    }
}

#[test]
fn status_timeout_uses_one_command_budget() {
    let mut reply = Reply::json(status(1));
    reply.delay_ms = 3000;
    let server = Server::new(vec![("status", reply)]);
    let started = std::time::Instant::now();
    let result = envelope(&server.run(&["status", "--timeout", "1"]), 5);
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    assert_eq!(result["error"]["code"], "timeout");
    assert!(result["data"].is_null());
    assert_eq!(server.seen().len(), 1);
}

#[test]
fn explicit_server_degradation_is_not_a_success_exit() {
    let mut state = status(1);
    state["readiness"] = json!("failed");
    let server = Server::new(vec![("status", Reply::json(state))]);
    let result = envelope(&server.run(&["status"]), 5);
    assert_eq!(result["error"]["code"], "status_failed");
    assert_eq!(result["data"]["readiness"], "failed");
}
