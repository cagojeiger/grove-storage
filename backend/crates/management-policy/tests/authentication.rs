mod support;

use grove_management_policy::*;
use support::*;

#[test]
fn invalid_credential_state_blocks_every_action() {
    for mut caller in [user(Role::Admin, AuthMethod::UserSession)] {
        for state in [
            CredentialState::Expired,
            CredentialState::Revoked,
            CredentialState::Invalid,
        ] {
            caller.credential_state = state;
            for method in METHODS {
                caller.method = method;
                for surface in SURFACES {
                    for action in ACTIONS {
                        assert_eq!(
                            authorize(caller, surface, action),
                            Err(Denial::InvalidCredential)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn disabled_or_deleted_accounts_cannot_use_any_action() {
    for state in [AccountState::Disabled, AccountState::Deleted] {
        for actor in [Actor::User {
            role: Role::Admin,
            state,
        }] {
            for method in METHODS {
                let caller = Caller {
                    actor,
                    method,
                    credential_state: CredentialState::Active,
                };
                for surface in SURFACES {
                    for action in ACTIONS {
                        assert_eq!(
                            authorize(caller, surface, action),
                            Err(Denial::InactiveAccount)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn authentication_method_and_surface_must_match_the_actor() {
    for mut caller in [user(Role::Admin, AuthMethod::UserSession)] {
        for method in METHODS {
            caller.method = method;
            for surface in SURFACES {
                let valid_context = matches!(
                    (caller.actor, method, surface),
                    (
                        Actor::User { .. },
                        AuthMethod::UserSession,
                        Surface::Console
                    ) | (
                        Actor::User { .. },
                        AuthMethod::ManagementToken,
                        Surface::Cli | Surface::Mcp | Surface::ResourceApi
                    )
                );
                for action in ACTIONS {
                    let result = authorize(caller, surface, action);
                    if valid_context {
                        assert_ne!(
                            result,
                            Err(Denial::InvalidAuthenticationContext),
                            "{caller:?} {surface:?}"
                        );
                    } else {
                        assert_eq!(
                            result,
                            Err(Denial::InvalidAuthenticationContext),
                            "{caller:?} {surface:?} {action:?}"
                        );
                    }
                }
            }
        }
    }
}
