mod support;

use grove_management_policy::*;
use support::*;

#[test]
fn console_matrix_preserves_role_and_query_scope() {
    for role in ROLES {
        for (action, viewer_operator, admin) in [
            (
                Action::ReadOwnSessions,
                Ok(Scope::SelfOnly),
                Ok(Scope::SelfOnly),
            ),
            (
                Action::RevokeOwnSessions,
                Ok(Scope::SelfOnly),
                Ok(Scope::SelfOnly),
            ),
            (
                Action::ReadIdentities,
                Err(Denial::InsufficientRole),
                Ok(Scope::Installation),
            ),
            (
                Action::ManageIdentities,
                Err(Denial::InsufficientRole),
                Ok(Scope::Installation),
            ),
            (
                Action::ReadAuditHistory,
                Ok(Scope::SelfAndOwnedAgents),
                Ok(Scope::Installation),
            ),
            (
                Action::ReadInvocationHistory,
                Ok(Scope::SelfAndOwnedAgents),
                Ok(Scope::Installation),
            ),
            (
                Action::ReadSecurityEvents,
                Err(Denial::InsufficientRole),
                Ok(Scope::Installation),
            ),
        ] {
            assert_eq!(
                authorize(
                    user(role, AuthMethod::UserSession),
                    Surface::Console,
                    action
                ),
                if role == Role::Admin {
                    admin
                } else {
                    viewer_operator
                },
                "{role:?} {action:?}"
            );
        }
    }
}

#[test]
fn even_admin_bearer_cannot_use_identity_session_or_history_operations() {
    for role in ROLES {
        for surface in MACHINE_SURFACES {
            for action in CONSOLE_ACTIONS {
                assert_eq!(
                    authorize(user(role, AuthMethod::ManagementToken), surface, action),
                    Err(Denial::ConsoleSessionRequired),
                    "{role:?} {surface:?} {action:?}"
                );
            }
        }
    }
}

#[test]
fn users_cannot_use_master_setup_or_recovery_even_as_admin() {
    for role in ROLES {
        for surface in SURFACES {
            let method = if surface == Surface::Console {
                AuthMethod::UserSession
            } else {
                AuthMethod::ManagementToken
            };
            for action in [Action::BootstrapAdmin, Action::RecoverAdmin] {
                assert_eq!(
                    authorize(user(role, method), surface, action),
                    Err(Denial::MasterSessionRequired)
                );
            }
        }
    }
}
