use filegate_core::LogFormat;
use tracing_subscriber::{EnvFilter, filter::filter_fn, prelude::*};

fn payload_safe(metadata: &tracing::Metadata<'_>) -> bool {
    // The SDK logs complete protocol payloads at debug/trace and may echo
    // peer-provided fields at other levels. Application logs own MCP diagnostics.
    metadata.target() != "rmcp" && !metadata.target().starts_with("rmcp::")
}

pub(crate) fn init(format: LogFormat) {
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(filter_fn(payload_safe));
    match format {
        LogFormat::Json => subscriber
            .with(tracing_subscriber::fmt::layer().json())
            .init(),
        LogFormat::Pretty => subscriber.with(tracing_subscriber::fmt::layer()).init(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::{Arc, Mutex},
    };

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn sdk_payload_logs_stay_disabled_even_with_trace_enabled() {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::new("trace,rmcp=trace,rmcp::service=trace"))
            .with(filter_fn(payload_safe))
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(move || writer.clone()),
            );
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target: "rmcp::service", payload="secret-canary");
            tracing::warn!(target: "rmcp::transport", payload="secret-canary");
            tracing::info!("application-safe-marker");
        });
        let bytes = capture.0.lock().unwrap_or_else(|error| error.into_inner());
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("secret-canary"));
        assert!(text.contains("application-safe-marker"));
    }
}
