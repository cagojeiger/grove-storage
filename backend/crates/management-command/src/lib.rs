//! Shared management command contracts; no HTTP, MCP runtime, or database I/O.
//!
//! Commands cover registry operations, not console identity/history operations.
//! Schema validation is followed by service-owned resource and storage checks.
//! Serializable inputs and issued credentials are wire data, never audit data.

#![forbid(unsafe_code)]

mod catalog;
mod error;
pub mod input;
pub mod metadata;
pub mod model;

pub use catalog::{Command, CommandName, Effect, Output};
pub use error::{CommandError, ErrorCode, Outcome};

/// Command compatibility is independent of package and CLI output versions.
pub const COMMAND_PROTOCOL_VERSION: u16 = 1;

/// Reject protocol and unknown commands before inspecting their payload.
pub fn decode(
    protocol: u16,
    name: &str,
    input: serde_json::Value,
) -> Result<Command, CommandError> {
    if protocol != COMMAND_PROTOCOL_VERSION {
        return Err(CommandError::rejected(ErrorCode::ProtocolIncompatible));
    }
    let name = CommandName::parse(name)
        .ok_or_else(|| CommandError::rejected(ErrorCode::UnknownCommand))?;
    if !input.is_object() {
        return Err(CommandError::rejected(ErrorCode::InvalidInput));
    }
    let command = name.decode_input(input)?;
    command.validate()?;
    Ok(command)
}
