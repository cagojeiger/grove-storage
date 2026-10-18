use super::{HttpLabels, Metrics};
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::Instant;

pub(crate) struct KnownMethods(Vec<String>);

impl Default for KnownMethods {
    fn default() -> Self {
        Self(
            [
                "GET", "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH",
                "QUERY",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        )
    }
}

impl KnownMethods {
    pub fn from_env() -> anyhow::Result<Self> {
        match std::env::var("OTEL_INSTRUMENTATION_HTTP_KNOWN_METHODS") {
            Ok(value) => Self::parse(&value),
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(_) => anyhow::bail!("OTEL_INSTRUMENTATION_HTTP_KNOWN_METHODS must be UTF-8"),
        }
    }

    fn parse(value: &str) -> anyhow::Result<Self> {
        let mut methods = Vec::new();
        if !value.is_empty() {
            for name in value.split(',').map(str::trim) {
                anyhow::ensure!(
                    !name.is_empty() && axum::http::Method::from_bytes(name.as_bytes()).is_ok(),
                    "OTEL_INSTRUMENTATION_HTTP_KNOWN_METHODS contains an invalid method"
                );
                if !methods.iter().any(|method| method == name) {
                    methods.push(name.to_owned());
                }
            }
        }
        Ok(Self(methods))
    }

    pub fn normalize(&self, method: &axum::http::Method) -> String {
        if self.0.iter().any(|known| known == method.as_str()) {
            method.as_str().to_owned()
        } else {
            "_OTHER".to_owned()
        }
    }
}

pub(crate) struct ServerRequest {
    metrics: Metrics,
    labels: HttpLabels,
    scheme: &'static str,
    is_head: bool,
    started: Instant,
    status: Option<u16>,
    finished: bool,
    log_span: Option<tracing::Span>,
    s3_error_code: Option<&'static str>,
}

impl ServerRequest {
    pub fn new(metrics: Metrics, labels: HttpLabels, scheme: &'static str, is_head: bool) -> Self {
        Self {
            metrics,
            labels,
            scheme,
            is_head,
            started: Instant::now(),
            status: None,
            finished: false,
            log_span: None,
            s3_error_code: None,
        }
    }

    pub fn log_to(&mut self, span: tracing::Span) {
        self.log_span = Some(span);
    }

    pub fn response(mut self, response: Response) -> Response {
        self.status = Some(response.status().as_u16());
        self.s3_error_code = response
            .extensions()
            .get::<crate::logging::http::S3ErrorCode>()
            .map(|code| code.0);
        let (parts, body) = response.into_parts();
        if self.is_head || matches!(parts.status.as_u16(), 100..=199 | 204 | 304) {
            self.finish(None);
            return Response::from_parts(parts, body);
        }
        let remaining = parts
            .headers
            .get(axum::http::header::CONTENT_LENGTH)
            .and_then(|length| length.to_str().ok())
            .and_then(|length| length.parse().ok())
            .or_else(|| body.size_hint().exact());
        if body.is_end_stream() {
            self.finish(remaining.filter(|n| *n > 0).map(|_| "body_length_mismatch"));
            return Response::from_parts(parts, body);
        }
        Response::from_parts(
            parts,
            Body::new(ObservedBody {
                body,
                request: self,
                remaining,
                data_complete: remaining == Some(0),
            }),
        )
    }

    fn finish(&mut self, error: Option<&'static str>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let elapsed = self.started.elapsed();
        self.metrics.http_server_finished(
            &self.labels,
            self.scheme,
            self.status,
            error,
            elapsed.as_secs_f64(),
        );
        if let Some(span) = &self.log_span {
            crate::logging::http::request_finished(
                span,
                self.labels.surface,
                self.status,
                error,
                self.s3_error_code,
                elapsed,
            );
        }
    }
}

impl Drop for ServerRequest {
    fn drop(&mut self) {
        self.finish(Some("interrupted"));
    }
}

struct ObservedBody {
    body: Body,
    request: ServerRequest,
    remaining: Option<u64>,
    data_complete: bool,
}

impl Drop for ObservedBody {
    fn drop(&mut self) {
        // Hyper may stop polling after Content-Length bytes without observing EOF.
        let error = (!self.data_complete).then_some("interrupted");
        self.request.finish(error);
    }
}

impl HttpBody for ObservedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let result = Pin::new(&mut self.body).poll_frame(cx);
        match &result {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref()
                    && let Some(remaining) = self.remaining
                {
                    if let Some(left) = remaining.checked_sub(data.len() as u64) {
                        self.remaining = Some(left);
                        self.data_complete |= left == 0 && !data.is_empty();
                    } else {
                        self.request.finish(Some("body_length_mismatch"));
                    }
                }
                if self.body.is_end_stream() {
                    let error = self
                        .remaining
                        .filter(|n| *n > 0)
                        .map(|_| "body_length_mismatch");
                    self.request.finish(error);
                }
            }
            Poll::Ready(Some(Err(_))) => self.request.finish(Some("body_error")),
            Poll::Ready(None) => {
                let error = self
                    .remaining
                    .filter(|n| *n > 0)
                    .map(|_| "body_length_mismatch");
                self.request.finish(error);
            }
            Poll::Pending => {}
        }
        result
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

#[cfg(test)]
mod tests;
