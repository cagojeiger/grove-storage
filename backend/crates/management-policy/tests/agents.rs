mod support;

use grove_management_policy::*;
use support::*;

#[test]
fn resource_write_requires_both_agent_and_owner_permission() {
    for role in AGENT_ROLES {
        for owner_role in ROLES {
            for surface in MACHINE_SURFACES {
                let caller = agent(role, owner_role);
                assert_eq!(
                    authorize(caller, surface, Action::ReadResources),
                    Ok(Scope::Installation)
                );
                let expected = match (role, owner_role) {
                    (AgentRole::Operator, Role::Operator | Role::Admin) => Ok(Scope::Installation),
                    _ => Err(Denial::InsufficientRole),
                };
                for action in [Action::WriteResources, Action::ManageServiceCredentials] {
                    assert_eq!(
                        authorize(caller, surface, action),
                        expected,
                        "{caller:?} {action:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn agents_cannot_use_console_operations_even_with_admin_owner() {
    for role in AGENT_ROLES {
        for owner_role in ROLES {
            for surface in MACHINE_SURFACES {
                for action in CONSOLE_ACTIONS {
                    assert_eq!(
                        authorize(agent(role, owner_role), surface, action),
                        Err(Denial::ConsoleSessionRequired)
                    );
                }
            }
        }
    }
}

#[test]
fn agents_cannot_bootstrap_or_recover_an_admin() {
    for role in AGENT_ROLES {
        for owner_role in ROLES {
            for surface in MACHINE_SURFACES {
                for action in [Action::BootstrapAdmin, Action::RecoverAdmin] {
                    assert_eq!(
                        authorize(agent(role, owner_role), surface, action),
                        Err(Denial::MasterSessionRequired)
                    );
                }
            }
        }
    }
}

#[test]
fn owner_demotion_changes_the_next_authorization() {
    let before = agent(AgentRole::Operator, Role::Admin);
    let after = agent(AgentRole::Operator, Role::Viewer);
    assert_eq!(
        authorize(before, Surface::Cli, Action::WriteResources),
        Ok(Scope::Installation)
    );
    assert_eq!(
        authorize(after, Surface::Cli, Action::WriteResources),
        Err(Denial::InsufficientRole)
    );
}

#[test]
fn inactive_owner_blocks_all_agent_actions() {
    for owner_state in [AccountState::Disabled, AccountState::Deleted] {
        let caller = Caller {
            actor: Actor::Agent {
                role: AgentRole::Operator,
                state: AccountState::Active,
                owner_role: Role::Admin,
                owner_state,
            },
            ..agent(AgentRole::Operator, Role::Admin)
        };
        for surface in SURFACES {
            for action in ACTIONS {
                assert_eq!(
                    authorize(caller, surface, action),
                    Err(Denial::InactiveOwner)
                );
            }
        }
    }
}
