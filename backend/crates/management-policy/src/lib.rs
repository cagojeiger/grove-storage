//! Pure management authorization, independent of HTTP, persistence, and secrets.
//!
//! Adapters supply verified credentials and current account/owner state. This
//! crate neither authenticates raw credentials nor proves browser interaction.
//! An allowed scope still requires resource guards and scoped database queries.

#![forbid(unsafe_code)]

mod identity;
mod policy;

pub use identity::{
    AccountState, Actor, AgentRole, AuthMethod, Caller, CredentialState, Role, Surface,
};
pub use policy::{Action, Denial, Scope, authorize};
