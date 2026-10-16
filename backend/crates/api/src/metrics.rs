use std::sync::Arc;

use grove_core::time::{Clock, SystemClock};
use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeLabelSet, EncodeMetric, text::encode},
    metrics::{
        counter::{ConstCounter, Counter},
        family::Family,
        gauge::Gauge,
        histogram::Histogram,
        info::Info,
    },
    registry::{Registry, Unit},
};
use tracing_appender::non_blocking::ErrorCounter;

pub(crate) mod database;
pub(crate) mod endpoint;
pub(crate) mod http;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct HttpLabels {
    pub surface: &'static str,
    pub method: String,
    pub route: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct HttpResult {
    #[prometheus(flatten)]
    request: HttpLabels,
    status_class: &'static str,
    outcome: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct TransferLabels {
    direction: &'static str,
    outcome: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct WorkerLabels {
    pub worker: &'static str,
    pub task: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct WorkerResult {
    #[prometheus(flatten)]
    job: WorkerLabels,
    outcome: &'static str,
}

#[derive(Clone)]
pub(crate) struct Metrics(Arc<Inner>);

struct Inner {
    registry: Registry,
    provider: grove_infra::S3Metrics,
    database: database::Cache,
    clock: Arc<dyn Clock>,
    http_methods: http::KnownMethods,
    http_requests: Family<HttpResult, Counter>,
    http_duration: Family<HttpLabels, Histogram>,
    http_server_duration: Family<Vec<(&'static str, String)>, Histogram>,
    http_active: Family<HttpLabels, Gauge>,
    transfers: Family<TransferLabels, Counter>,
    transfer_bytes: Family<Vec<(&'static str, &'static str)>, Counter>,
    transfer_duration: Family<TransferLabels, Histogram>,
    transfer_active: Family<Vec<(&'static str, &'static str)>, Gauge>,
    worker_runs: Family<WorkerResult, Counter>,
    worker_duration: Family<WorkerLabels, Histogram>,
    worker_active: Family<WorkerLabels, Gauge>,
    worker_last_success: Family<WorkerLabels, Gauge>,
    worker_processed: Family<WorkerLabels, Counter>,
    worker_failures: Family<WorkerLabels, Counter>,
    worker_skipped: Family<WorkerLabels, Counter>,
    pool_connections: Family<Vec<(&'static str, &'static str)>, Gauge>,
    pool_idle: Family<Vec<(&'static str, &'static str)>, Gauge>,
    pool_limit: Family<Vec<(&'static str, &'static str)>, Gauge>,
    spool_reserved: Gauge,
    spool_capacity: Gauge,
    spool_active: Gauge,
}

fn duration_histogram() -> Histogram {
    Histogram::new([
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 30.0, 60.0, 300.0,
        1800.0, 7200.0,
    ])
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new(Arc::new(SystemClock), None)
    }
}

impl Metrics {
    pub fn new(clock: Arc<dyn Clock>, log_errors: Option<ErrorCounter>) -> Self {
        Self::with_http_methods(clock, log_errors, http::KnownMethods::default())
    }

    pub fn with_http_methods(
        clock: Arc<dyn Clock>,
        log_errors: Option<ErrorCounter>,
        http_methods: http::KnownMethods,
    ) -> Self {
        let mut inner = Inner {
            registry: Registry::default(),
            provider: grove_infra::S3Metrics::default(),
            database: database::Cache::default(),
            clock,
            http_methods,
            http_requests: Family::default(),
            http_duration: Family::new_with_constructor(duration_histogram),
            http_server_duration: Family::new_with_constructor(|| {
                // Recommended HTTP buckets plus the storage-transfer latency tail.
                Histogram::new([
                    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5,
                    10.0, 30.0, 60.0, 300.0, 1800.0, 7200.0,
                ])
            }),
            http_active: Family::default(),
            transfers: Family::default(),
            transfer_bytes: Family::default(),
            transfer_duration: Family::new_with_constructor(duration_histogram),
            transfer_active: Family::default(),
            worker_runs: Family::default(),
            worker_duration: Family::new_with_constructor(duration_histogram),
            worker_active: Family::default(),
            worker_last_success: Family::default(),
            worker_processed: Family::default(),
            worker_failures: Family::default(),
            worker_skipped: Family::default(),
            pool_connections: Family::default(),
            pool_idle: Family::default(),
            pool_limit: Family::default(),
            spool_reserved: Gauge::default(),
            spool_capacity: Gauge::default(),
            spool_active: Gauge::default(),
        };
        let r = &mut inner.registry;
        r.register_with_unit(
            "http_server_request_duration",
            "HTTP server request duration through local response body completion or interruption",
            Unit::Seconds,
            inner.http_server_duration.clone(),
        );
        let r = r.sub_registry_with_prefix("grove");
        inner.provider.register(r);
        r.register(
            "http_requests",
            "Requests ending in response creation or interruption",
            inner.http_requests.clone(),
        );
        r.register_with_unit(
            "http_response_creation_duration",
            "Time until response creation or interruption, not body delivery",
            Unit::Seconds,
            inner.http_duration.clone(),
        );
        r.register(
            "http_requests_in_flight",
            "Requests awaiting response creation",
            inner.http_active.clone(),
        );
        r.register(
            "transfers",
            "Observed relay streams by terminal outcome",
            inner.transfers.clone(),
        );
        r.register_with_unit(
            "transfer_observed",
            "Bytes read from receive bodies or provided to the HTTP send runtime",
            Unit::Bytes,
            inner.transfer_bytes.clone(),
        );
        r.register_with_unit(
            "transfer_duration",
            "Time until relay stream terminal observation",
            Unit::Seconds,
            inner.transfer_duration.clone(),
        );
        r.register(
            "transfers_in_flight",
            "Observed relay streams not yet terminated",
            inner.transfer_active.clone(),
        );
        r.register(
            "worker_runs",
            "Worker invocations by terminal outcome",
            inner.worker_runs.clone(),
        );
        r.register_with_unit(
            "worker_duration",
            "Time until worker terminal observation",
            Unit::Seconds,
            inner.worker_duration.clone(),
        );
        r.register(
            "worker_runs_in_flight",
            "Worker invocations not yet terminated",
            inner.worker_active.clone(),
        );
        r.register_with_unit(
            "worker_last_success_timestamp",
            "Last completed pass without observed failure; zero means never",
            Unit::Seconds,
            inner.worker_last_success.clone(),
        );
        r.register(
            "worker_processed",
            "Confirmed processed or deleted entries reported by completed passes",
            inner.worker_processed.clone(),
        );
        r.register(
            "worker_failures",
            "Failure observations reported by completed passes, not unique entries",
            inner.worker_failures.clone(),
        );
        r.register(
            "worker_skipped",
            "Deferred entries or ownership attempts reported by completed passes",
            inner.worker_skipped.clone(),
        );
        r.register(
            "db_pool_connections",
            "Open connections in the process pool",
            inner.pool_connections.clone(),
        );
        r.register(
            "db_pool_idle_connections",
            "Idle connections in the process pool",
            inner.pool_idle.clone(),
        );
        r.register(
            "db_pool_max_connections",
            "Configured process pool connection limit",
            inner.pool_limit.clone(),
        );
        r.register_with_unit(
            "spool_reserved",
            "Live rounded transfer reservations, not physical filesystem usage",
            Unit::Bytes,
            inner.spool_reserved.clone(),
        );
        r.register_with_unit(
            "spool_capacity",
            "Effective rounded transfer reservation budget",
            Unit::Bytes,
            inner.spool_capacity.clone(),
        );
        r.register(
            "spool_transfers_in_flight",
            "Held transfer admission slots",
            inner.spool_active.clone(),
        );
        r.register(
            "build",
            "Running Grove build",
            Info::new(vec![("version", env!("CARGO_PKG_VERSION"))]),
        );
        r.register_collector(Box::new(LogDrops(log_errors)));
        r.register_collector(Box::new(inner.database.clone()));
        for direction in ["receive", "send"] {
            drop(
                inner
                    .transfer_active
                    .get_or_create(&vec![("direction", direction)]),
            );
            drop(
                inner
                    .transfer_bytes
                    .get_or_create(&vec![("direction", direction)]),
            );
            for outcome in [
                "success",
                "incomplete",
                "error",
                "interrupted",
                "timeout",
                "too_slow",
                "rejected",
            ] {
                let labels = TransferLabels { direction, outcome };
                drop(inner.transfers.get_or_create(&labels));
            }
        }
        Self(Arc::new(inner))
    }

    pub fn provider(&self) -> grove_infra::S3Metrics {
        self.0.provider.clone()
    }

    pub fn http_method(&self, method: &axum::http::Method) -> String {
        self.0.http_methods.normalize(method)
    }

    pub fn http_started(&self, labels: &HttpLabels) {
        self.0.http_active.get_or_create(labels).inc();
    }

    pub fn http_finished(
        &self,
        labels: &HttpLabels,
        status: Option<u16>,
        outcome: &'static str,
        seconds: f64,
    ) {
        let status_class = match status {
            Some(100..=199) => "1xx",
            Some(200..=299) => "2xx",
            Some(300..=399) => "3xx",
            Some(400..=499) => "4xx",
            Some(500..=599) => "5xx",
            _ => "none",
        };
        self.0
            .http_requests
            .get_or_create(&HttpResult {
                request: labels.clone(),
                status_class,
                outcome,
            })
            .inc();
        self.0.http_duration.get_or_create(labels).observe(seconds);
        self.0.http_active.get_or_create(labels).dec();
    }

    pub fn http_server_finished(
        &self,
        request: &HttpLabels,
        scheme: &'static str,
        status: Option<u16>,
        error: Option<&'static str>,
        seconds: f64,
    ) {
        let mut labels = vec![
            ("http_request_method", request.method.to_owned()),
            ("url_scheme", scheme.to_owned()),
        ];
        if !request.route.is_empty() {
            labels.push(("http_route", request.route.clone()));
        }
        if let Some(status) = status {
            labels.push(("http_response_status_code", status.to_string()));
        }
        if let Some(error) = error {
            labels.push(("error_type", error.to_owned()));
        } else if let Some(status @ 500..=599) = status {
            labels.push(("error_type", status.to_string()));
        }
        self.0
            .http_server_duration
            .get_or_create(&labels)
            .observe(seconds);
    }

    pub fn transfer_started(&self, direction: &'static str) -> Counter {
        self.0
            .transfer_active
            .get_or_create(&vec![("direction", direction)])
            .inc();
        self.0
            .transfer_bytes
            .get_or_create_owned(&vec![("direction", direction)])
    }

    pub fn transfer_finished(&self, direction: &'static str, outcome: &'static str, seconds: f64) {
        let labels = TransferLabels { direction, outcome };
        self.0.transfers.get_or_create(&labels).inc();
        self.0
            .transfer_duration
            .get_or_create(&labels)
            .observe(seconds);
        self.0
            .transfer_active
            .get_or_create(&vec![("direction", direction)])
            .dec();
    }

    pub fn register_worker(&self, labels: &WorkerLabels) {
        drop(self.0.worker_active.get_or_create(labels));
        drop(self.0.worker_last_success.get_or_create(labels));
        drop(self.0.worker_processed.get_or_create(labels));
        drop(self.0.worker_failures.get_or_create(labels));
        drop(self.0.worker_skipped.get_or_create(labels));
        for outcome in ["success", "error", "timeout", "skipped", "interrupted"] {
            drop(self.0.worker_runs.get_or_create(&WorkerResult {
                job: labels.clone(),
                outcome,
            }));
        }
    }

    pub fn worker_started(&self, labels: &WorkerLabels) {
        self.0.worker_active.get_or_create(labels).inc();
        drop(self.0.worker_last_success.get_or_create(labels));
    }

    pub fn worker_finished(
        &self,
        labels: &WorkerLabels,
        outcome: &'static str,
        seconds: f64,
        report: Option<(u64, u64, u64)>,
    ) {
        self.0
            .worker_runs
            .get_or_create(&WorkerResult {
                job: labels.clone(),
                outcome,
            })
            .inc();
        self.0
            .worker_duration
            .get_or_create(labels)
            .observe(seconds);
        self.0.worker_active.get_or_create(labels).dec();
        if outcome == "success" {
            self.0
                .worker_last_success
                .get_or_create(labels)
                .set(self.0.clock.now().timestamp());
        }
        if let Some((processed, failed, skipped)) = report {
            self.0
                .worker_processed
                .get_or_create(labels)
                .inc_by(processed);
            self.0.worker_failures.get_or_create(labels).inc_by(failed);
            self.0.worker_skipped.get_or_create(labels).inc_by(skipped);
        }
    }

    pub fn render(&self) -> Result<String, std::fmt::Error> {
        let mut body = String::new();
        encode(&mut body, &self.0.registry)?;
        Ok(body)
    }
}

#[derive(Debug)]
struct LogDrops(Option<ErrorCounter>);

impl Collector for LogDrops {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        let value = ConstCounter::new(self.0.as_ref().map_or(0, ErrorCounter::dropped_lines));
        let metric = encoder.encode_descriptor(
            "log_dropped_lines",
            "Process stdout queue overflow; not downstream delivery failures",
            None,
            value.metric_type(),
        )?;
        value.encode(metric)
    }
}
