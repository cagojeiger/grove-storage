#![allow(clippy::expect_used)]

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_grove-storage"))
        .args(args)
        .env_remove("GROVE_DATABASE_URL")
        .env_remove("GROVE_ENC_ROOT_SECRET")
        .output()
        .expect("run server command")
}

#[test]
fn help_distinguishes_local_server_commands_from_remote_cli() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stderr);
    for command in ["[serve]", "status", "account", "openapi"] {
        assert!(help.contains(&format!("grove-storage {command}")));
    }
    assert!(help.contains("gscli --help"));
    assert!(help.contains("GROVE_*"));
    assert!(!help.contains("filegate "));
    assert!(!help.contains("grove-storage admin"));
}

#[test]
fn account_help_needs_no_database_and_has_no_remote_login() {
    let output = run(&["account", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("grove-storage account init"));
    assert!(help.contains("grove-storage account recover"));
    assert!(help.contains("GROVE_DATABASE_URL"));
    assert!(help.contains("privately"));
    assert!(!help.contains("filegate "));
}

#[test]
fn server_does_not_offer_remote_management_or_self_update() {
    for command in ["update", "storage", "client", "admin"] {
        let output = run(&[command]);
        assert_eq!(output.status.code(), Some(2));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(&format!("grove-storage: unknown command '{command}'")));
    }
}

#[test]
fn conflicting_console_and_object_hosts_fail_before_filesystem_or_database_access() {
    let output = Command::new(env!("CARGO_BIN_EXE_grove-storage"))
        .arg("serve")
        .env(
            "GROVE_DATABASE_URL",
            "postgres://unused:unused@127.0.0.1:1/unused",
        )
        .env(
            "GROVE_ENC_ROOT_SECRET",
            "local-test-root-secret-at-least-32bytes",
        )
        .env("GROVE_CONSOLE_ORIGIN", "https://console.test")
        .env("GROVE_PUBLIC_URL", "https://CONSOLE.TEST:443/objects")
        .env("GROVE_CONSOLE_DIST_DIR", "/missing-grove-console-build")
        .output()
        .expect("run conflicting server configuration");
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("GROVE_PUBLIC_URL must not use the console host"),
        "{error}"
    );
}
