//! S3 completion rules over the measured part ledger; no storage or DB I/O.

pub const MIN_PART_BYTES: i64 = 5 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum CompletionError {
    InvalidPart,
    InvalidPartOrder,
    EntityTooSmall,
}

impl CompletionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidPart => "InvalidPart",
            Self::InvalidPartOrder => "InvalidPartOrder",
            Self::EntityTooSmall => "EntityTooSmall",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::InvalidPart => "a listed part is invalid, missing or has a mismatched etag",
            Self::InvalidPartOrder => "parts must be in ascending order without duplicates",
            Self::EntityTooSmall => "each non-final part must be at least 5 MiB",
        }
    }
}

pub fn reconcile(
    client_parts: &[(i32, String)],
    ledger: &[(i32, i64, String)],
) -> Result<Vec<(i32, i64, String)>, CompletionError> {
    if client_parts.is_empty() || client_parts.iter().any(|(n, _)| !(1..=10000).contains(n)) {
        return Err(CompletionError::InvalidPart);
    }
    if client_parts
        .windows(2)
        .any(|pair| matches!(pair, [a, b] if a.0 >= b.0))
    {
        return Err(CompletionError::InvalidPartOrder);
    }
    let by_number: std::collections::HashMap<_, _> = ledger
        .iter()
        .map(|(number, size, etag)| (*number, (*size, etag.as_str())))
        .collect();
    let mut completion = Vec::with_capacity(client_parts.len());
    for (number, etag) in client_parts {
        let (size, recorded) = by_number.get(number).ok_or(CompletionError::InvalidPart)?;
        if *size < 0 || !recorded.eq_ignore_ascii_case(etag.trim_matches('"')) {
            return Err(CompletionError::InvalidPart);
        }
        completion.push((*number, *size, (*recorded).to_owned()));
    }
    // The last requested part is exempt, even if the ledger has later parts.
    if completion
        .iter()
        .take(completion.len() - 1)
        .any(|(_, size, _)| *size < MIN_PART_BYTES)
    {
        return Err(CompletionError::EntityTooSmall);
    }
    Ok(completion)
}
