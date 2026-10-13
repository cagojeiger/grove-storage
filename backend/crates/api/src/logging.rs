//! Operational observations are independent of durable management audit.
mod format;
pub(crate) mod http;
#[cfg(test)]
mod tests;
pub(crate) mod transfer;
pub(crate) mod worker;

use grove_core::{LogFormat, time::Clock};
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, filter::filter_fn, prelude::*};

pub(crate) fn payload_safe(metadata: &tracing::Metadata<'_>) -> bool {
    // Dependencies can echo protocol payloads even above DEBUG.
    metadata.target().starts_with("grove_")
}

pub(crate) fn init(
    mode: LogFormat,
    clock: Arc<dyn Clock>,
) -> tracing_appender::non_blocking::WorkerGuard {
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(1024)
        .lossy(true)
        .finish(std::io::stdout());
    let errors = writer.error_counter();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(filter_fn(payload_safe))
        .with(
            tracing_subscriber::fmt::layer()
                .fmt_fields(format::SafeFields)
                .event_format(format::SafeEvent {
                    mode,
                    clock,
                    errors: Some(errors),
                })
                .with_writer(writer),
        )
        .init();
    guard
}
