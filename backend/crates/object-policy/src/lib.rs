//! Pure upload declarations, multipart geometry, and ETag rules.
//!
//! Callers provide validated configuration and recorded part checksums.
//! This crate performs no I/O and owns no runtime or persistence state.

pub mod completion;
pub mod multipart;
pub mod validation;
