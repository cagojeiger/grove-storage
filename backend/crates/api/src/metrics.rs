//! Process metrics are independent of management identity and database availability.

mod snapshot;
mod stream;
#[cfg(test)]
mod tests;

pub(crate) use snapshot::sample;
pub(crate) use stream::{Transfer, download_body};

use axum::{Router, extract::Request, middleware::Next, response::Response, routing::get};
use grove_s3_protocol::operation::{Operation, classify};
use metrics::{Counter, Gauge, Histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tokio::time::Instant;

pub(crate) fn builder() -> anyhow::Result<PrometheusBuilder> {
    Ok(PrometheusBuilder::new().set_buckets(&[
        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 300.0,
        900.0, 3600.0, 86400.0,
    ])?)
}

pub(crate) fn install() -> anyhow::Result<PrometheusHandle> {
    let handle = builder()?.install_recorder()?;
    for name in [
        "grove_http_response_duration_seconds",
        "grove_transfer_duration_seconds",
        "grove_provider_duration_seconds",
        "grove_job_duration_seconds",
        "grove_retention_duration_seconds",
        "grove_management_duration_seconds",
    ] {
        metrics::describe_histogram!(
            name,
            metrics::Unit::Seconds,
            "Observed elapsed time; see the Grove metrics contract for completion semantics."
        );
    }
    for name in [
        "grove_http_requests_total",
        "grove_transfer_bytes_total",
        "grove_transfers_total",
        "grove_spool_rejections_total",
        "grove_provider_requests_total",
        "grove_jobs_total",
        "grove_job_items_total",
        "grove_worker_timeouts_total",
        "grove_snapshot_errors_total",
        "grove_retention_passes_total",
        "grove_retention_rows_deleted_total",
        "grove_management_operations_total",
        "grove_history_write_errors_total",
    ] {
        metrics::describe_counter!(
            name,
            "Process-local observed events; restarts reset counters."
        );
    }
    for name in [
        "grove_http_in_flight",
        "grove_transfer_in_flight",
        "grove_spool_slots",
        "grove_spool_budget_bytes",
        "grove_spool_reserved_bytes",
        "grove_spool_file_bytes",
        "grove_spool_files",
        "grove_db_pool_connections",
        "grove_job_in_flight",
        "grove_job_last_completed_timestamp_seconds",
        "grove_job_last_success_timestamp_seconds",
        "grove_snapshot_available",
        "grove_snapshot_last_success_timestamp_seconds",
        "grove_objects",
        "grove_objects_bytes",
        "grove_recovery_queue_objects",
        "grove_recovery_queue_oldest_timestamp_seconds",
        "grove_recovery_queue_unknown_age_objects",
        "grove_retention_last_success_timestamp_seconds",
        "grove_retention_oldest_remaining_timestamp_seconds",
        "grove_process_started_timestamp_seconds",
        "grove_build_info",
    ] {
        metrics::describe_gauge!(
            name,
            "Process observation or cached database state; see the Grove metrics contract."
        );
    }
    for operation in [
        "put",
        "get",
        "head",
        "delete",
        "multipart_create",
        "multipart_part",
        "multipart_complete",
        "multipart_abort",
        "preflight",
        "unsupported",
    ] {
        for outcome in ["2xx", "4xx", "5xx", "cancelled"] {
            metrics::counter!("grove_http_requests_total", "surface" => "s3",
                "operation" => operation, "outcome" => outcome)
            .increment(0);
        }
    }
    for source in ["db", "spool"] {
        metrics::gauge!("grove_snapshot_available", "source" => source).set(0.0);
        metrics::gauge!("grove_snapshot_last_success_timestamp_seconds", "source" => source)
            .set(0.0);
        metrics::counter!("grove_snapshot_errors_total", "source" => source).increment(0);
    }
    for (direction, outcomes) in [
        (
            "receive",
            &[
                "success",
                "error",
                "interrupted",
                "timeout",
                "too_slow",
                "too_large",
                "incomplete",
            ][..],
        ),
        (
            "send",
            &["success", "error", "interrupted", "incomplete"][..],
        ),
    ] {
        metrics::counter!("grove_transfer_bytes_total", "direction" => direction).increment(0);
        metrics::gauge!("grove_transfer_in_flight", "direction" => direction).set(0.0);
        for outcome in outcomes {
            metrics::counter!("grove_transfers_total", "direction" => direction,
                "outcome" => *outcome)
            .increment(0);
        }
    }
    for reason in ["slots", "bytes"] {
        metrics::counter!("grove_spool_rejections_total", "reason" => reason).increment(0);
    }
    for stream in ["invocations", "security"] {
        metrics::counter!("grove_history_write_errors_total", "stream" => stream).increment(0);
    }
    metrics::gauge!("grove_build_info", "version" => env!("CARGO_PKG_VERSION")).set(1.0);
    crate::reconciler::initialize_metrics();
    metrics::gauge!("grove_job_last_success_timestamp_seconds", "worker" => "management", "task" => "retention").set(0.0);
    for stream in [
        grove_db::retention::Stream::Audit,
        grove_db::retention::Stream::Security,
        grove_db::retention::Stream::Invocations,
        grove_db::retention::Stream::Sessions,
        grove_db::retention::Stream::ApiTokens,
        grove_db::retention::Stream::PasswordSetup,
        grove_db::retention::Stream::ObjectAccess,
        grove_db::retention::Stream::Usage,
    ] {
        metrics::gauge!("grove_retention_last_success_timestamp_seconds", "stream" => stream.name()).set(0.0);
    }
    Ok(handle)
}

pub(crate) fn router(handle: PrometheusHandle) -> Router {
    Router::new().route(
        "/metrics",
        get(move || {
            let handle = handle.clone();
            async move {
                handle.run_upkeep();
                (
                    [
                        ("content-type", "text/plain; version=0.0.4; charset=utf-8"),
                        ("cache-control", "no-store"),
                    ],
                    handle.render(),
                )
            }
        }),
    )
}

fn labels(request: &Request) -> (&'static str, &'static str) {
    let path = request.uri().path();
    if path == "/" || crate::routes::is_system_path(path) {
        return ("system", "probe");
    }
    if path.starts_with("/api/admin/console") {
        return ("console", "asset");
    }
    if path.starts_with("/api/admin/") {
        return ("management", "request");
    }
    if path.starts_with("/api/") || path.starts_with("/blobs/") {
        return ("native", "request");
    }
    if request.method() == axum::http::Method::OPTIONS {
        return ("s3", "preflight");
    }
    let operation = match classify(
        request.method().as_str(),
        request.uri().query().unwrap_or(""),
        request.headers().contains_key("x-amz-copy-source"),
    ) {
        Ok(Operation::Put) => "put",
        Ok(Operation::Get) => "get",
        Ok(Operation::Head) => "head",
        Ok(Operation::Delete) => "delete",
        Ok(Operation::CreateMultipart) => "multipart_create",
        Ok(Operation::UploadPart { .. }) => "multipart_part",
        Ok(Operation::CompleteMultipart { .. }) => "multipart_complete",
        Ok(Operation::AbortMultipart { .. }) => "multipart_abort",
        Err(_) => "unsupported",
    };
    ("s3", operation)
}

pub(crate) async fn observe_request(request: Request, next: Next) -> Response {
    let (surface, operation) = labels(&request);
    let mut observation = RequestObservation::new(surface, operation);
    let response = next.run(request).await;
    observation.finish(match response.status().as_u16() / 100 {
        1 => "1xx",
        2 => "2xx",
        3 => "3xx",
        4 => "4xx",
        _ => "5xx",
    });
    response
}

struct RequestObservation {
    started: Instant,
    surface: &'static str,
    operation: &'static str,
    in_flight: Gauge,
    duration: Histogram,
    finished: bool,
}

impl RequestObservation {
    fn new(surface: &'static str, operation: &'static str) -> Self {
        let in_flight = metrics::gauge!("grove_http_in_flight", "surface" => surface);
        in_flight.increment(1.0);
        Self {
            started: Instant::now(),
            surface,
            operation,
            in_flight,
            duration: metrics::histogram!("grove_http_response_duration_seconds",
                "surface" => surface, "operation" => operation),
            finished: false,
        }
    }

    fn finish(&mut self, outcome: &'static str) {
        if self.finished {
            return;
        }
        self.finished = true;
        metrics::counter!("grove_http_requests_total", "surface" => self.surface,
            "operation" => self.operation, "outcome" => outcome)
        .increment(1);
        self.duration.record(self.started.elapsed().as_secs_f64());
        self.in_flight.decrement(1.0);
    }
}

impl Drop for RequestObservation {
    fn drop(&mut self) {
        self.finish("cancelled");
    }
}

#[derive(Default)]
pub(crate) struct JobReport {
    pub processed: u64,
    pub failed: u64,
    pub skipped: u64,
}

pub(crate) struct JobObservation {
    started: Instant,
    worker: &'static str,
    job: &'static str,
    finished: bool,
    duration: Histogram,
    in_flight: Gauge,
}

impl JobObservation {
    pub fn new(worker: &'static str, job: &'static str) -> Self {
        let in_flight = metrics::gauge!("grove_job_in_flight", "worker" => worker, "task" => job);
        in_flight.increment(1.0);
        Self {
            started: Instant::now(),
            worker,
            job,
            finished: false,
            in_flight,
            duration: metrics::histogram!("grove_job_duration_seconds", "worker" => worker, "task" => job),
        }
    }

    pub fn finish(&mut self, report: &JobReport, now: f64) {
        if self.finished {
            return;
        }
        let outcome = if report.failed == 0 {
            "success"
        } else {
            "error"
        };
        self.record(outcome);
        for (outcome, count) in [
            ("processed", report.processed),
            ("failed", report.failed),
            ("skipped", report.skipped),
        ] {
            metrics::counter!("grove_job_items_total", "worker" => self.worker,
                "task" => self.job, "outcome" => outcome)
            .increment(count);
        }
        metrics::gauge!("grove_job_last_completed_timestamp_seconds", "worker" => self.worker, "task" => self.job).set(now);
        if report.failed == 0 {
            metrics::gauge!("grove_job_last_success_timestamp_seconds", "worker" => self.worker, "task" => self.job).set(now);
        }
    }

    pub fn skipped(&mut self) {
        self.record("skipped");
    }

    fn record(&mut self, outcome: &'static str) {
        if self.finished {
            return;
        }
        self.finished = true;
        let counter: Counter = metrics::counter!("grove_jobs_total", "worker" => self.worker,
            "task" => self.job, "outcome" => outcome);
        counter.increment(1);
        self.duration.record(self.started.elapsed().as_secs_f64());
        self.in_flight.decrement(1.0);
    }
}

impl Drop for JobObservation {
    fn drop(&mut self) {
        self.record("interrupted");
    }
}
