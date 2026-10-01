#![allow(clippy::unwrap_used)]
#[allow(dead_code)]
#[path = "../src/args.rs"]
mod args;

use clap::{CommandFactory, Parser};
use grove_management_command::CommandName;
use std::collections::BTreeSet;

fn leaves(command: &clap::Command, prefix: &str, found: &mut BTreeSet<String>) {
    for child in command.get_subcommands() {
        if child.is_hide_set() || (prefix.is_empty() && child.get_name() == "update") {
            continue;
        }
        let name = if prefix.is_empty() {
            child.get_name().to_owned()
        } else {
            format!("{prefix}.{}", child.get_name())
        };
        if child.get_subcommands().next().is_some() {
            leaves(child, &name, found);
        } else {
            found.insert(name);
        }
    }
}

#[test]
fn every_remote_cli_leaf_exists_in_the_shared_catalog_and_nothing_else() {
    let mut actual = BTreeSet::new();
    leaves(&args::Args::command(), "", &mut actual);
    let expected: BTreeSet<String> = CommandName::ALL
        .iter()
        .map(|name| name.as_str().to_owned())
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 24);
}

#[test]
fn parsed_cli_names_match_the_catalog_including_hyphenated_commands() {
    let cases: &[&[&str]] = &[
        &["status"],
        &["storage", "list"],
        &["storage", "metadata", "show", "s"],
        &[
            "storage",
            "metadata",
            "replace",
            "s",
            "--from",
            "metadata.json",
        ],
        &["client", "metadata", "show", "c"],
        &[
            "client",
            "metadata",
            "replace",
            "c",
            "--from",
            "metadata.json",
        ],
        &["storage", "show", "s"],
        &["storage", "test", "s"],
        &["storage", "create", "s", "--from", "input.json"],
        &["storage", "replace", "s", "--from", "input.json"],
        &["storage", "delete", "s"],
        &["client", "list"],
        &["client", "show", "c"],
        &["client", "create", "c", "--storage", "s"],
        &["client", "delete", "c"],
        &["credential", "list", "--client", "c"],
        &[
            "credential",
            "create",
            "--client",
            "c",
            "--secret-out",
            "secret.json",
        ],
        &["credential", "delete", "--client", "c", "fgak12345678"],
        &["client-key", "list", "--client", "c"],
        &[
            "client-key",
            "register",
            "--client",
            "c",
            "--key-file",
            "key",
        ],
        &["usage", "storages"],
        &["usage", "clients"],
        &["usage", "history"],
    ];
    let mut names = BTreeSet::new();
    for case in cases {
        let parsed =
            args::Args::try_parse_from(std::iter::once("gscli").chain(case.iter().copied()))
                .unwrap();
        assert!(CommandName::parse(parsed.command.name()).is_some());
        names.insert(parsed.command.name());
    }
    let hash = format!("sha256:{}", "a".repeat(64));
    let parsed =
        args::Args::try_parse_from(["gscli", "client-key", "delete", "--client", "c", &hash])
            .unwrap();
    assert_eq!(parsed.command.name(), CommandName::ClientKeyDelete.as_str());
    names.insert(parsed.command.name());
    assert_eq!(
        names,
        CommandName::ALL.iter().map(|name| name.as_str()).collect()
    );
}
