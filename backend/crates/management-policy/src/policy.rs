use crate::{AccountState, Actor, AgentRole, AuthMethod, Caller, CredentialState, Role, Surface};

/// Management operations only; runtime Client file access has a separate policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ReadResources,
    WriteResources,
    ManageServiceCredentials,
    ReadOwnSessions,
    RevokeOwnSessions,
    ReadIdentities,
    ManageIdentities,
    ReadAuditHistory,
    ReadInvocationHistory,
    ReadSecurityEvents,
    BootstrapAdmin,
    RecoverAdmin,
    ManageSetupSession,
}

/// The service must apply this scope using authenticated IDs, not owner IDs
/// supplied by the request. Installation scope still requires resource guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Installation,
    SelfOnly,
    SelfAndOwnedAgents,
    SetupRecovery,
}

/// Internal policy reasons, not raw authentication responses for public clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denial {
    InvalidCredential,
    InactiveAccount,
    InactiveOwner,
    InvalidAuthenticationContext,
    ConsoleSessionRequired,
    InsufficientRole,
    MasterSessionRequired,
    MasterScopeOnly,
}

/// Decide an operation's scope from a verified, current authentication snapshot.
/// Setup/recovery admission does not replace transactional bootstrap checks.
pub fn authorize(caller: Caller, surface: Surface, action: Action) -> Result<Scope, Denial> {
    if caller.credential_state != CredentialState::Active {
        return Err(Denial::InvalidCredential);
    }
    let role = effective_role(caller.actor)?;
    if !matches!(
        (caller.actor, caller.method, surface),
        (
            Actor::User { .. },
            AuthMethod::UserSession,
            Surface::Console
        ) | (
            Actor::User { .. } | Actor::Agent { .. },
            AuthMethod::ManagementToken,
            Surface::Cli | Surface::Mcp | Surface::ResourceApi
        ) | (Actor::Master, AuthMethod::MasterSession, Surface::Console)
    ) {
        return Err(Denial::InvalidAuthenticationContext);
    }

    let Some(role) = role else {
        return match action {
            Action::BootstrapAdmin | Action::RecoverAdmin | Action::ManageSetupSession => {
                Ok(Scope::SetupRecovery)
            }
            _ => Err(Denial::MasterScopeOnly),
        };
    };

    match action {
        Action::ReadResources => Ok(Scope::Installation),
        Action::WriteResources | Action::ManageServiceCredentials => match role {
            Role::Viewer => Err(Denial::InsufficientRole),
            Role::Operator | Role::Admin => Ok(Scope::Installation),
        },
        Action::ReadOwnSessions | Action::RevokeOwnSessions => {
            require_console_session(caller)?;
            Ok(Scope::SelfOnly)
        }
        Action::ReadIdentities | Action::ManageIdentities | Action::ReadSecurityEvents => {
            require_console_session(caller)?;
            match role {
                Role::Admin => Ok(Scope::Installation),
                Role::Viewer | Role::Operator => Err(Denial::InsufficientRole),
            }
        }
        Action::ReadAuditHistory | Action::ReadInvocationHistory => {
            require_console_session(caller)?;
            Ok(match role {
                Role::Admin => Scope::Installation,
                Role::Viewer | Role::Operator => Scope::SelfAndOwnedAgents,
            })
        }
        Action::BootstrapAdmin | Action::RecoverAdmin | Action::ManageSetupSession => {
            Err(Denial::MasterSessionRequired)
        }
    }
}

fn require_console_session(caller: Caller) -> Result<(), Denial> {
    if caller.method != AuthMethod::UserSession {
        return Err(Denial::ConsoleSessionRequired);
    }
    Ok(())
}

fn effective_role(actor: Actor) -> Result<Option<Role>, Denial> {
    match actor {
        Actor::Master => Ok(None),
        Actor::User { role, state } => {
            require_active(state)?;
            Ok(Some(role))
        }
        Actor::Agent {
            role,
            state,
            owner_role,
            owner_state,
        } => {
            require_active(state)?;
            if owner_state != AccountState::Active {
                return Err(Denial::InactiveOwner);
            }
            Ok(Some(match (role, owner_role) {
                (AgentRole::Viewer, _) | (_, Role::Viewer) => Role::Viewer,
                (AgentRole::Operator, Role::Operator | Role::Admin) => Role::Operator,
            }))
        }
    }
}

fn require_active(state: AccountState) -> Result<(), Denial> {
    if state != AccountState::Active {
        return Err(Denial::InactiveAccount);
    }
    Ok(())
}
