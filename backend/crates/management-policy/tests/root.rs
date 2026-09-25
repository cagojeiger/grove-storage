mod support;
use grove_management_policy::*;
use support::*;

#[test]
fn root_requires_its_own_console_session() {
    for method in [
        AuthMethod::RootSession,
        AuthMethod::MasterSession,
        AuthMethod::UserSession,
        AuthMethod::ManagementToken,
    ] {
        for surface in SURFACES {
            for action in ACTIONS {
                let caller = Caller {
                    actor: Actor::Root,
                    method,
                    credential_state: CredentialState::Active,
                };
                let expected = if method != AuthMethod::RootSession || surface != Surface::Console {
                    Err(Denial::InvalidAuthenticationContext)
                } else if matches!(
                    action,
                    Action::BootstrapAdmin | Action::RecoverAdmin | Action::ManageSetupSession
                ) {
                    Err(Denial::MasterSessionRequired)
                } else {
                    Ok(Scope::Installation)
                };
                assert_eq!(authorize(caller, surface, action), expected);
                for state in [
                    CredentialState::Revoked,
                    CredentialState::Expired,
                    CredentialState::Invalid,
                ] {
                    assert_eq!(
                        authorize(
                            Caller {
                                credential_state: state,
                                ..caller
                            },
                            surface,
                            action
                        ),
                        Err(Denial::InvalidCredential)
                    );
                }
            }
        }
    }
}
