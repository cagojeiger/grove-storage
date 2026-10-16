use std::future::Future;

use aws_sdk_s3::error::SdkError;
use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, family::Family, gauge::Gauge, histogram::Histogram},
    registry::{Registry, Unit},
};
use tokio::time::Instant;

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    HeadBucket,
    ListMultipartUploads,
    PutObject,
    GetObject,
    DeleteObject,
    HeadObject,
    CreateMultipartUpload,
    UploadPart,
    ListParts,
    CompleteMultipartUpload,
    AbortMultipartUpload,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::HeadBucket => "head_bucket",
            Self::ListMultipartUploads => "list_multipart_uploads",
            Self::PutObject => "put_object",
            Self::GetObject => "get_object",
            Self::DeleteObject => "delete_object",
            Self::HeadObject => "head_object",
            Self::CreateMultipartUpload => "create_multipart_upload",
            Self::UploadPart => "upload_part",
            Self::ListParts => "list_parts",
            Self::CompleteMultipartUpload => "complete_multipart_upload",
            Self::AbortMultipartUpload => "abort_multipart_upload",
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct Call {
    operation: &'static str,
    status_class: &'static str,
    outcome: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct Labels {
    operation: &'static str,
}

#[derive(Clone, Debug)]
pub struct S3Metrics {
    calls: Family<Call, Counter>,
    duration: Family<Labels, Histogram>,
    active: Family<Labels, Gauge>,
}

impl Default for S3Metrics {
    fn default() -> Self {
        Self {
            calls: Family::default(),
            duration: Family::new_with_constructor(|| {
                Histogram::new([
                    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 30.0,
                    60.0, 300.0, 1800.0, 7200.0,
                ])
            }),
            active: Family::default(),
        }
    }
}

impl S3Metrics {
    pub fn register(&self, registry: &mut Registry) {
        registry.register(
            "provider_calls",
            "Logical S3 SDK sends including internal retries; GET ends at response headers",
            self.calls.clone(),
        );
        registry.register_with_unit(
            "provider_call_duration",
            "Time until logical SDK send returns or is dropped; not GET body delivery",
            Unit::Seconds,
            self.duration.clone(),
        );
        registry.register(
            "provider_calls_in_flight",
            "Logical S3 SDK sends not yet returned or dropped",
            self.active.clone(),
        );
    }

    pub(crate) async fn observe<T, E>(
        &self,
        operation: Operation,
        future: impl Future<Output = Result<T, SdkError<E>>>,
    ) -> Result<T, SdkError<E>> {
        let labels = Labels {
            operation: operation.name(),
        };
        self.active.get_or_create(&labels).inc();
        let mut observation = Observation {
            metrics: self.clone(),
            labels,
            started: Instant::now(),
            finished: false,
        };
        let result = future.await;
        let (status_class, outcome) = match &result {
            Ok(_) => ("2xx", "success"),
            Err(SdkError::TimeoutError(_)) => ("none", "timeout"),
            Err(SdkError::DispatchFailure(error)) if error.is_timeout() => ("none", "timeout"),
            Err(error) => {
                let status = error.raw_response().map(|r| r.status().as_u16());
                let class = match status {
                    Some(100..=199) => "1xx",
                    Some(200..=299) => "2xx",
                    Some(300..=399) => "3xx",
                    Some(400..=499) => "4xx",
                    Some(500..=599) => "5xx",
                    _ => "none",
                };
                (
                    class,
                    if status == Some(404) {
                        "not_found"
                    } else {
                        "error"
                    },
                )
            }
        };
        observation.finish(status_class, outcome);
        result
    }
}

struct Observation {
    metrics: S3Metrics,
    labels: Labels,
    started: Instant,
    finished: bool,
}

impl Observation {
    fn finish(&mut self, status_class: &'static str, outcome: &'static str) {
        self.finished = true;
        self.metrics
            .calls
            .get_or_create(&Call {
                operation: self.labels.operation,
                status_class,
                outcome,
            })
            .inc();
        self.metrics
            .duration
            .get_or_create(&self.labels)
            .observe(self.started.elapsed().as_secs_f64());
        self.metrics.active.get_or_create(&self.labels).dec();
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        if !self.finished {
            self.finish("none", "interrupted");
        }
    }
}

#[cfg(test)]
mod tests;
