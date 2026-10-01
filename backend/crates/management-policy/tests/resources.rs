mod support;

use grove_management_policy::*;
use support::*;

#[test]
fn resource_roles_match_on_console_cli_mcp_and_direct_api() {
    for role in ROLES {
        for surface in SURFACES {
            let method = if surface == Surface::Console {
                AuthMethod::UserSession
            } else {
                AuthMethod::ManagementToken
            };
            for (action, writable) in [
                (Action::ReadResources, false),
                (Action::WriteResources, true),
                (Action::ManageServiceCredentials, true),
            ] {
                let expected = if writable && role == Role::Reader {
                    Err(Denial::InsufficientRole)
                } else {
                    Ok(Scope::Installation)
                };
                assert_eq!(
                    authorize(user(role, method), surface, action),
                    expected,
                    "{role:?} {surface:?} {action:?}"
                );
            }
        }
    }
}

#[test]
fn cli_and_mcp_agree_for_every_action_and_credential_context() {
    for role in ROLES {
        let mut caller = user(role, AuthMethod::ManagementToken);
        for method in METHODS {
            caller.method = method;
            for action in ACTIONS {
                assert_eq!(
                    authorize(caller, Surface::Cli, action),
                    authorize(caller, Surface::Mcp, action),
                    "{caller:?} {action:?}"
                );
            }
        }
    }
}
