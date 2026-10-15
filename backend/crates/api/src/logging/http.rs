use crate::metrics::{HttpLabels, Metrics};
use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};
use tokio::time::Instant;
use tracing::{Instrument, Span};

struct Observation {
    started: Instant,
    span: Span,
    finished: bool,
    metrics: Metrics,
    labels: HttpLabels,
}

impl Drop for Observation {
    fn drop(&mut self) {
        if !self.finished {
            self.metrics.http_finished(
                &self.labels,
                None,
                "interrupted",
                self.started.elapsed().as_secs_f64(),
            );
            tracing::info!(parent: &self.span, event = "http.interrupted", outcome = "interrupted",
                duration_ms = self.started.elapsed().as_millis() as u64);
        }
    }
}

pub(crate) async fn observe(
    State(metrics): State<Metrics>,
    request: Request,
    next: Next,
) -> Response {
    let method = method(request.method());
    let path = request.uri().path();
    let probe = crate::routes::is_system_path(path);
    let surface = if probe || path == "/" {
        "system"
    } else if path.starts_with("/api/admin/console") || path.starts_with("/api/admin/identity/") {
        "console"
    } else if path == "/api/admin/mcp" {
        "mcp"
    } else if path.starts_with("/api/admin/commands/") {
        "resource_api"
    } else if path.starts_with("/api/admin") {
        "management"
    } else if path.starts_with("/api/") || path.starts_with("/blobs/") {
        "native"
    } else {
        "s3"
    };
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("");
    let request_id = uuid::Uuid::new_v4();
    // A context-only ERROR span keeps correlation when the event filter is ERROR.
    // Request headers are untouched: a caller can include x-request-id in SigV4.
    let span = tracing::error_span!("request", %request_id, method, surface, route);
    let labels = HttpLabels {
        surface,
        method,
        route: route.to_owned(),
    };
    metrics.http_started(&labels);
    let mut observation = Observation {
        started: Instant::now(),
        span: span.clone(),
        finished: false,
        metrics,
        labels,
    };
    let response = next.run(request).instrument(span.clone()).await;
    observation.finished = true;
    let duration_ms = observation.started.elapsed().as_millis() as u64;
    let status = response.status().as_u16();
    let operation_id = response
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| uuid::Uuid::parse_str(h).ok());
    let operation_id = operation_id.map(|id| id.to_string()).unwrap_or_default();
    let unsupported = surface == "s3" && status == 501;
    let expected_rejection = surface == "s3" && matches!(status, 404 | 412);
    let outcome = if status >= 500 && !unsupported {
        "error"
    } else if status >= 400 {
        "rejected"
    } else {
        "success"
    };
    observation.metrics.http_finished(
        &observation.labels,
        Some(status),
        outcome,
        observation.started.elapsed().as_secs_f64(),
    );
    if status >= 500 && !unsupported {
        tracing::error!(parent: &span, event = "http.response", status, duration_ms, operation_id, outcome);
    } else if expected_rejection {
        tracing::debug!(parent: &span, event = "http.response", status, duration_ms, operation_id, outcome);
    } else if status >= 400 && !probe {
        tracing::warn!(parent: &span, event = "http.response", status, duration_ms, operation_id, outcome);
    } else if !probe && duration_ms >= 1000 {
        tracing::info!(parent: &span, event = "http.response", status, duration_ms, operation_id, outcome);
    } else {
        tracing::debug!(parent: &span, event = "http.response", status, duration_ms, operation_id, outcome);
    }
    response
}

fn method(method: &axum::http::Method) -> &'static str {
    match method.as_str() {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "OPTIONS" => "OPTIONS",
        _ => "OTHER",
    }
}
