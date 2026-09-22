//! S3 wire-format parsing, independent of HTTP servers and storage state.

pub mod multipart;
pub mod operation;
pub mod signing;
