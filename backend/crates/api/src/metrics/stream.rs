use std::{
    pin::Pin,
    task::{Context, Poll},
};

use axum::body::{Body, Bytes};
use http_body::{Body as HttpBody, Frame, SizeHint};
use metrics::{Counter, Gauge};
use tokio::time::Instant;

pub(crate) struct Transfer {
    direction: &'static str,
    started: Instant,
    bytes: Counter,
    in_flight: Gauge,
    written: u64,
    finished: bool,
}

impl Transfer {
    pub fn new(direction: &'static str) -> Self {
        let in_flight = metrics::gauge!("grove_transfer_in_flight", "direction" => direction);
        in_flight.increment(1.0);
        Self {
            direction,
            started: Instant::now(),
            written: 0,
            finished: false,
            in_flight,
            bytes: metrics::counter!("grove_transfer_bytes_total", "direction" => direction),
        }
    }

    pub fn bytes(&mut self, count: usize) {
        self.written += count as u64;
        self.bytes.increment(count as u64);
    }

    pub fn finish(&mut self, outcome: &'static str) {
        if self.finished {
            return;
        }
        self.finished = true;
        metrics::counter!("grove_transfers_total", "direction" => self.direction,
            "outcome" => outcome)
        .increment(1);
        metrics::histogram!("grove_transfer_duration_seconds", "direction" => self.direction,
            "outcome" => outcome)
        .record(self.started.elapsed().as_secs_f64());
        self.in_flight.decrement(1.0);
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        self.finish("interrupted");
    }
}

struct Download {
    inner: Body,
    expected: u64,
    observation: Transfer,
}

pub(crate) fn download_body(body: Body, expected: i64) -> Body {
    let mut observation = Transfer::new("send");
    if expected == 0 {
        observation.finish("success");
    }
    Body::new(Download {
        inner: body,
        expected: expected.max(0) as u64,
        observation,
    })
}

impl HttpBody for Download {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let result = Pin::new(&mut self.inner).poll_frame(cx);
        match &result {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    self.observation.bytes(data.len());
                }
                // HTTP may stop polling once Content-Length bytes have been emitted.
                if self.observation.written >= self.expected || self.inner.is_end_stream() {
                    let outcome = if self.observation.written == self.expected {
                        "success"
                    } else {
                        "incomplete"
                    };
                    self.observation.finish(outcome);
                }
            }
            Poll::Ready(Some(Err(_))) => self.observation.finish("error"),
            Poll::Ready(None) => {
                let outcome = if self.observation.written == self.expected {
                    "success"
                } else {
                    "incomplete"
                };
                self.observation.finish(outcome);
            }
            Poll::Pending => {}
        }
        result
    }

    fn is_end_stream(&self) -> bool {
        self.observation.finished && self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
