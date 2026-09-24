#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Viewer,
    Operator,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountState {
    Active,
    Disabled,
    Deleted,
}

/// Effective credential state, including the parent token for a user session
/// and the current configuration generation for a master session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialState {
    Active,
    Expired,
    Revoked,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    User { role: Role, state: AccountState },
    Master,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    UserSession,
    ManagementToken,
    MasterSession,
}

/// Selected by server routing, never from a client-provided channel header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Console,
    Cli,
    Mcp,
    ResourceApi,
}

/// Trusted authentication snapshot; never deserialize this from request input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caller {
    pub actor: Actor,
    pub method: AuthMethod,
    pub credential_state: CredentialState,
}
