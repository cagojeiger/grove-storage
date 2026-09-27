//! Object control-plane orchestration, independent of HTTP and provider I/O.
//! Operations coordinate physical effects with object metadata transitions;
//! registry configuration and account changes belong to management.

pub mod cleanup;
pub mod multipart_create;
