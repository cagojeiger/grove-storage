use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ProtocolIncompatible,
    UnknownCommand,
    InvalidInput,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    RequestRejected,
    InvalidResponse,
    Unavailable,
    Internal,
}

/// Evidence about a mutation, independent of its transport status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    NotApplied,
    Applied,
    Unknown,
}

/// Safe structured error data. Free-form provider/serde errors stay outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandError {
    pub code: ErrorCode,
    pub outcome: Outcome,
}

impl CommandError {
    /// For errors proved to occur before a mutation. Transport failures after
    /// dispatch must instead retain Applied or Unknown evidence.
    pub const fn rejected(code: ErrorCode) -> Self {
        Self {
            code,
            outcome: Outcome::NotApplied,
        }
    }
}
