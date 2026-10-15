use crate::metrics::Metrics;
use axum::body::{Body, Bytes};
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::Instant;
use tracing::Span;

pub(crate) struct Transfer {
    direction: &'static str,
    expected: u64,
    observed: u64,
    started: Instant,
    span: Span,
    finished: bool,
    metrics: Metrics,
    bytes_counter: prometheus_client::metrics::counter::Counter,
}

impl Transfer {
    pub fn new(direction: &'static str, expected: i64, metrics: Metrics) -> Self {
        let bytes_counter = metrics.transfer_started(direction);
        Self {
            direction,
            expected: expected.max(0) as u64,
            observed: 0,
            started: Instant::now(),
            span: Span::current(),
            finished: false,
            metrics,
            bytes_counter,
        }
    }
    pub fn bytes(&mut self, count: usize) {
        self.observed += count as u64;
        self.bytes_counter.inc_by(count as u64);
    }
    pub fn finish(&mut self, outcome: &'static str, error_code: &'static str) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.metrics.transfer_finished(
            self.direction,
            outcome,
            self.started.elapsed().as_secs_f64(),
        );
        let expected_bytes = self.expected;
        let observed_bytes = self.observed;
        let direction = self.direction;
        let duration_ms = self.started.elapsed().as_millis() as u64;
        if outcome == "success" {
            tracing::debug!(parent: &self.span, event = "transfer.finished", direction, outcome,
                expected_bytes, observed_bytes, duration_ms);
        } else if outcome == "interrupted" {
            tracing::info!(parent: &self.span, event = "transfer.finished", direction, outcome,
                expected_bytes, observed_bytes, duration_ms, error_code);
        } else if matches!(
            error_code,
            "body_read_failed" | "spool_write_failed" | "spool_flush_failed"
        ) {
            tracing::error!(parent: &self.span, event = "transfer.finished", direction, outcome,
                expected_bytes, observed_bytes, duration_ms, error_code);
        } else {
            tracing::warn!(parent: &self.span, event = "transfer.finished", direction, outcome,
                expected_bytes, observed_bytes, duration_ms, error_code);
        }
    }
}
impl Drop for Transfer {
    fn drop(&mut self) {
        self.finish("interrupted", "stream_dropped");
    }
}

struct Download {
    inner: Body,
    transfer: Transfer,
}

pub(crate) fn download(body: Body, expected: i64, file_id: uuid::Uuid, metrics: Metrics) -> Body {
    let mut transfer = Transfer::new("send", expected, metrics);
    transfer.span = tracing::error_span!("transfer", %file_id);
    if expected == 0 {
        transfer.finish("success", "");
    }
    Body::new(Download {
        inner: body,
        transfer,
    })
}

impl HttpBody for Download {
    type Data = Bytes;
    type Error = axum::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let result = Pin::new(&mut self.inner).poll_frame(cx);
        match &result {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    self.transfer.bytes(data.len());
                }
                if self.transfer.observed >= self.transfer.expected || self.inner.is_end_stream() {
                    self.complete();
                }
            }
            Poll::Ready(Some(Err(_))) => self.transfer.finish("error", "body_read_failed"),
            Poll::Ready(None) => self.complete(),
            Poll::Pending => {}
        }
        result
    }
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
impl Download {
    fn complete(&mut self) {
        if self.transfer.observed == self.transfer.expected {
            self.transfer.finish("success", "");
        } else {
            self.transfer.finish("incomplete", "length_mismatch");
        }
    }
}
