use grove_management_command::{CommandError, ErrorCode, Outcome};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: &'static str,
    pub message: &'static str,
    pub http_status: Option<u16>,
    pub outcome: &'static str,
    #[serde(skip)]
    pub exit: u8,
}

impl Error {
    pub fn input(message: &'static str) -> Self {
        Self::new("invalid_config", message, 2)
    }

    pub fn new(code: &'static str, message: &'static str, exit: u8) -> Self {
        Self {
            code,
            message,
            http_status: None,
            outcome: "not_applied",
            exit,
        }
    }

    pub fn response(status: u16) -> Self {
        let (code, message, exit) = match status {
            401 | 403 => (
                "unauthorized",
                "Management authentication or authorization failed",
                3,
            ),
            404 => ("not_found", "Resource not found", 4),
            409 => ("conflict", "Resource conflict", 6),
            400..=499 => ("request_rejected", "The API rejected the request", 7),
            300..=399 => ("redirect", "Redirects are not followed", 5),
            500..=599 => ("server_error", "The API returned a server error", 5),
            _ => ("invalid_response", "Unexpected HTTP status", 5),
        };
        Self {
            http_status: Some(status),
            ..Self::new(code, message, exit)
        }
    }

    pub fn untrusted_response(status: u16, mutation: bool) -> Self {
        let mut error = Self::response(status);
        if mutation {
            error.exit = 8;
            error.outcome = "unknown";
        }
        error
    }

    pub fn unverified_result(status: u16, mutation: bool) -> Self {
        let mut error = Self::new(
            "invalid_response",
            "Response does not match the command contract",
            5,
        );
        error.http_status = Some(status);
        if mutation {
            error.exit = 8;
            error.outcome = "unknown";
        }
        error
    }

    pub fn command_status(code: ErrorCode) -> u16 {
        match code {
            ErrorCode::Unauthorized => 401,
            ErrorCode::Forbidden => 403,
            ErrorCode::NotFound => 404,
            ErrorCode::Conflict => 409,
            ErrorCode::Unavailable => 503,
            ErrorCode::Internal | ErrorCode::InvalidResponse => 500,
            _ => 400,
        }
    }

    pub fn command(command: CommandError, status: u16) -> Self {
        let (code, message, exit) = match command.code {
            ErrorCode::ProtocolIncompatible => (
                "protocol_incompatible",
                "Server command protocol is incompatible",
                7,
            ),
            ErrorCode::UnknownCommand => {
                ("unknown_command", "Server does not support this command", 7)
            }
            ErrorCode::InvalidInput => ("invalid_input", "The API rejected the command input", 7),
            ErrorCode::Unauthorized => ("unauthorized", "Management authentication failed", 3),
            ErrorCode::Forbidden => ("forbidden", "Management permission denied", 3),
            ErrorCode::NotFound => ("not_found", "Resource not found", 4),
            ErrorCode::Conflict => ("conflict", "Resource conflict", 6),
            ErrorCode::RequestRejected => ("request_rejected", "The API rejected the request", 7),
            ErrorCode::InvalidResponse => ("invalid_response", "Invalid command response", 5),
            ErrorCode::Unavailable => ("unavailable", "Management service unavailable", 5),
            ErrorCode::Internal => ("internal", "Management service failed", 5),
        };
        Self {
            code,
            message,
            http_status: Some(status),
            outcome: match command.outcome {
                Outcome::NotApplied => "not_applied",
                Outcome::Applied => "applied",
                Outcome::Unknown => "unknown",
            },
            exit: if command.outcome == Outcome::NotApplied {
                exit
            } else {
                8
            },
        }
    }

    pub fn mutation_transport() -> Self {
        let mut error = Self::new("transport", "HTTP connection, TLS, or transport failure", 8);
        error.outcome = "unknown";
        error
    }

    pub fn mutation_timeout() -> Self {
        let mut error = Self::new("timeout", "The command HTTP deadline was exceeded", 8);
        error.outcome = "unknown";
        error
    }

    pub fn applied_invalid_response(status: u16) -> Self {
        let mut error = Self::new(
            "invalid_response",
            "Response does not match the API contract",
            8,
        );
        error.http_status = Some(status);
        error.outcome = "applied";
        error
    }

    pub fn applied_secret_write() -> Self {
        let mut error = Self::new(
            "secret_write_failed",
            "Credential was issued but its secret file is incomplete",
            8,
        );
        error.outcome = "applied";
        error
    }

    pub fn response_too_large(status: u16, mutation: bool) -> Self {
        let mut error = Self::new("response_too_large", "Response exceeds 8 MiB", 5);
        error.http_status = Some(status);
        if mutation {
            error.exit = 8;
            error.outcome = "unknown";
        }
        error
    }

    pub fn timeout() -> Self {
        Self::new("timeout", "The command HTTP deadline was exceeded", 5)
    }
}
