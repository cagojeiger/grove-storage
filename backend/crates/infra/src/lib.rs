//! Transfer coordination and disposable local spools.
//! Backend operations consume resolved settings; registry writes, authorization,
//! and object metadata transitions belong to their callers.

pub mod backend;
pub mod temp_spool;

// Keep existing adapters source-compatible while the SDK stays confined to its crate.
pub use grove_storage_provider::*;
