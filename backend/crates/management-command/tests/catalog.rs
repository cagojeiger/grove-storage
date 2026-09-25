#![allow(clippy::unwrap_used)]
mod support;

use grove_management_command::*;
use grove_management_policy::*;
use serde_json::json;

#[test]
fn catalog_names_are_unique_and_every_command_has_typed_input_and_output() {
    let mut names = std::collections::BTreeSet::new();
    for &name in CommandName::ALL {
        assert!(names.insert(name.as_str()));
        assert_eq!(CommandName::parse(name.as_str()), Some(name));
        let command = decode(
            COMMAND_PROTOCOL_VERSION,
            name.as_str(),
            support::input(name),
        )
        .unwrap();
        assert_eq!(command.name(), name);
        let output = name.decode_output(support::output(name)).unwrap();
        assert_eq!(output.name(), name);
    }
    assert_eq!(names.len(), 20);
}

#[test]
fn identity_history_runtime_and_local_commands_are_not_exposed() {
    for name in [
        "user.create",
        "agent.create",
        "role.update",
        "token.create",
        "session.revoke",
        "audit.list",
        "security.list",
        "master.recover",
        "file.upload",
        "update",
        "update.check",
        "install",
    ] {
        assert!(CommandName::parse(name).is_none());
        assert_eq!(
            decode(1, name, json!({})).unwrap_err(),
            CommandError::rejected(ErrorCode::UnknownCommand)
        );
    }
}

#[test]
fn permissions_and_mutation_effects_are_explicit_for_all_names() {
    for &name in CommandName::ALL {
        let expected_action = match name {
            CommandName::CredentialList
            | CommandName::CredentialCreate
            | CommandName::CredentialDelete
            | CommandName::ClientKeyList
            | CommandName::ClientKeyRegister
            | CommandName::ClientKeyDelete => Action::ManageServiceCredentials,
            CommandName::StorageCreate
            | CommandName::StorageReplace
            | CommandName::StorageDelete
            | CommandName::ClientCreate
            | CommandName::ClientDelete => Action::WriteResources,
            _ => Action::ReadResources,
        };
        assert_eq!(name.required_action(), expected_action);
        let expected_effect = match name {
            CommandName::StorageCreate
            | CommandName::StorageReplace
            | CommandName::StorageDelete
            | CommandName::ClientCreate
            | CommandName::ClientDelete
            | CommandName::CredentialCreate
            | CommandName::CredentialDelete
            | CommandName::ClientKeyRegister
            | CommandName::ClientKeyDelete => Effect::Mutation,
            _ => Effect::Read,
        };
        assert_eq!(name.effect(), expected_effect);
    }
    // Listing service keys is not a write, but still needs writer privilege.
    assert_eq!(CommandName::CredentialList.effect(), Effect::Read);
    assert_eq!(
        CommandName::CredentialList.required_action(),
        Action::ManageServiceCredentials
    );
}

#[test]
fn every_resource_command_has_identical_cli_and_mcp_authorization() {
    for role in [Role::Reader, Role::Writer, Role::Admin] {
        let caller = Caller {
            actor: Actor::User {
                role,
                state: AccountState::Active,
            },
            method: AuthMethod::ManagementToken,
            credential_state: CredentialState::Active,
        };
        for &name in CommandName::ALL {
            let cli = authorize(caller, Surface::Cli, name.required_action());
            assert_eq!(cli, authorize(caller, Surface::Mcp, name.required_action()));
            let allowed = role != Role::Reader || name.required_action() == Action::ReadResources;
            assert_eq!(cli.is_ok(), allowed, "{role:?} {name:?}");
        }
    }
}

#[test]
fn generated_input_schemas_are_objects_and_keep_local_options_out() {
    for &name in CommandName::ALL {
        let schema = serde_json::to_value(name.input_schema()).unwrap();
        assert_eq!(schema.get("type"), Some(&json!("object")), "{name:?}");
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&json!(false)),
            "{name:?}"
        );
        let properties = schema.get("properties").and_then(|value| value.as_object());
        for local in [
            "from",
            "key_file",
            "secret_out",
            "yes",
            "role",
            "token",
            "actor",
        ] {
            assert!(
                !properties.is_some_and(|fields| fields.contains_key(local)),
                "{name:?}: {local}"
            );
        }
        assert!(
            serde_json::to_value(name.output_schema())
                .unwrap()
                .is_object()
        );
    }
}
