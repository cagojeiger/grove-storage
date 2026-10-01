#![allow(dead_code)]

use grove_management_policy::*;

pub const ROLES: [Role; 3] = [Role::Reader, Role::Writer, Role::Admin];
pub const METHODS: [AuthMethod; 2] = [AuthMethod::UserSession, AuthMethod::ManagementToken];
pub const SURFACES: [Surface; 4] = [
    Surface::Console,
    Surface::Cli,
    Surface::Mcp,
    Surface::ResourceApi,
];
pub const MACHINE_SURFACES: [Surface; 3] = [Surface::Cli, Surface::Mcp, Surface::ResourceApi];
pub const ACTIONS: [Action; 10] = [
    Action::ReadResources,
    Action::WriteResources,
    Action::ManageServiceCredentials,
    Action::ReadOwnSessions,
    Action::RevokeOwnSessions,
    Action::ReadIdentities,
    Action::ManageIdentities,
    Action::ReadAuditHistory,
    Action::ReadInvocationHistory,
    Action::ReadSecurityEvents,
];
pub const CONSOLE_ACTIONS: [Action; 7] = [
    Action::ReadOwnSessions,
    Action::RevokeOwnSessions,
    Action::ReadIdentities,
    Action::ManageIdentities,
    Action::ReadAuditHistory,
    Action::ReadInvocationHistory,
    Action::ReadSecurityEvents,
];

pub fn user(role: Role, method: AuthMethod) -> Caller {
    Caller {
        actor: Actor::User {
            role,
            state: AccountState::Active,
        },
        method,
        credential_state: CredentialState::Active,
    }
}
