use crate::{AccountState, Actor, AuthMethod, Caller, CredentialState, Role, Surface};

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
}

/// The service must apply this scope using authenticated IDs, not owner IDs
/// supplied by the request. Installation scope still requires resource guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Installation,
    SelfOnly,
}

/// Internal policy reasons, not raw authentication responses for public clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denial {
    InvalidCredential,
    InactiveAccount,
    InvalidAuthenticationContext,
    ConsoleSessionRequired,
    InsufficientRole,
}

/// Decide an operation's scope from a verified, current authentication snapshot.
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
            Actor::User { .. },
            AuthMethod::ManagementToken,
            Surface::Cli | Surface::Mcp | Surface::ResourceApi
        )
    ) {
        return Err(Denial::InvalidAuthenticationContext);
    }

    match action {
        Action::ReadResources => Ok(Scope::Installation),
        Action::WriteResources | Action::ManageServiceCredentials => match role {
            Role::Reader => Err(Denial::InsufficientRole),
            Role::Writer | Role::Admin => Ok(Scope::Installation),
        },
        Action::ReadOwnSessions | Action::RevokeOwnSessions => {
            require_console_session(caller)?;
            Ok(Scope::SelfOnly)
        }
        Action::ReadIdentities | Action::ManageIdentities | Action::ReadSecurityEvents => {
            require_console_session(caller)?;
            match role {
                Role::Admin => Ok(Scope::Installation),
                Role::Reader | Role::Writer => Err(Denial::InsufficientRole),
            }
        }
        Action::ReadAuditHistory | Action::ReadInvocationHistory => {
            require_console_session(caller)?;
            Ok(match role {
                Role::Admin => Scope::Installation,
                Role::Reader | Role::Writer => Scope::SelfOnly,
            })
        }
    }
}

fn require_console_session(caller: Caller) -> Result<(), Denial> {
    if caller.method != AuthMethod::UserSession {
        return Err(Denial::ConsoleSessionRequired);
    }
    Ok(())
}

fn effective_role(actor: Actor) -> Result<Role, Denial> {
    match actor {
        Actor::User { role, state } => {
            require_active(state)?;
            Ok(role)
        }
    }
}

fn require_active(state: AccountState) -> Result<(), Denial> {
    if state != AccountState::Active {
        return Err(Denial::InactiveAccount);
    }
    Ok(())
}
