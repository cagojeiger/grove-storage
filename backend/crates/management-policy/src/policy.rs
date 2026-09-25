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
    SetupRecovery,
}

/// Internal policy reasons, not raw authentication responses for public clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denial {
    InvalidCredential,
    InactiveAccount,
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
            Actor::User { .. },
            AuthMethod::ManagementToken,
            Surface::Cli | Surface::Mcp | Surface::ResourceApi
        ) | (Actor::Master, AuthMethod::MasterSession, Surface::Console)
            | (Actor::Root, AuthMethod::RootSession, Surface::Console)
    ) {
        return Err(Denial::InvalidAuthenticationContext);
    }

    if caller.actor == Actor::Root {
        return match action {
            Action::BootstrapAdmin | Action::RecoverAdmin | Action::ManageSetupSession => {
                Err(Denial::MasterSessionRequired)
            }
            _ => Ok(Scope::Installation),
        };
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
        Actor::Master | Actor::Root => Ok(None),
        Actor::User { role, state } => {
            require_active(state)?;
            Ok(Some(role))
        }
    }
}

fn require_active(state: AccountState) -> Result<(), Denial> {
    if state != AccountState::Active {
        return Err(Denial::InactiveAccount);
    }
    Ok(())
}
