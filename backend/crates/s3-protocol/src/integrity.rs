//! Request checksum comparison and unsupported conditional-request detection.

use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Debug, PartialEq, Eq)]
pub enum DigestError {
    InvalidDigest,
    BadDigest,
}

impl DigestError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidDigest => "InvalidDigest",
            Self::BadDigest => "BadDigest",
        }
    }
}

pub fn verify_base64(expected: &str, actual: &[u8]) -> Result<(), DigestError> {
    let decoded = STANDARD
        .decode(expected)
        .map_err(|_| DigestError::InvalidDigest)?;
    if decoded.len() != actual.len() {
        return Err(DigestError::InvalidDigest);
    }
    if decoded != actual {
        return Err(DigestError::BadDigest);
    }
    Ok(())
}

pub fn verify_hex(expected: &str, actual: &str) -> Result<(), DigestError> {
    let bytes = hex::decode(actual).map_err(|_| DigestError::BadDigest)?;
    verify_base64(expected, &bytes)
}

pub fn is_conditional_header(name: &str) -> bool {
    matches!(
        name,
        "if-match"
            | "if-none-match"
            | "if-modified-since"
            | "if-unmodified-since"
            | "if-range"
            | "x-amz-if-match-size"
            | "x-amz-if-match-last-modified-time"
    )
}

/// Called only after resolving an existing immutable object version.
pub fn if_match_matches(value: &str, etag: Option<&str>) -> bool {
    if value.trim() == "*" {
        return true;
    }
    let Some(etag) = etag else {
        return false;
    };
    let quoted = format!("\"{etag}\"");
    value.split(',').any(|candidate| candidate.trim() == quoted)
}
