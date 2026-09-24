//! Completion recovery decisions; callers enforce ownership atomically.

pub struct ObjectObservation {
    pub size: i64,
    /// Filesystems without ETag metadata provide None.
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionAction {
    Finalize,
    Reopen,
    Cleanup,
}

/// Observation failures remain errors, never absence or a cleanup decision.
pub fn completion_action<E>(
    expected_size: i64,
    expected_etag: &str,
    multipart: bool,
    observation: Result<Option<ObjectObservation>, E>,
) -> Result<CompletionAction, E> {
    Ok(match observation? {
        Some(observed)
            if observed.size == expected_size
                && observed
                    .etag
                    .as_deref()
                    .is_none_or(|etag| etag.eq_ignore_ascii_case(expected_etag)) =>
        {
            CompletionAction::Finalize
        }
        None if multipart => CompletionAction::Reopen,
        _ => CompletionAction::Cleanup,
    })
}
