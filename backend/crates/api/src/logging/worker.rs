use crate::metrics::{Metrics, WorkerLabels};
use tokio::time::Instant;
use tracing::Span;

#[derive(Default)]
pub(crate) struct Report {
    pub processed: u64,
    pub failed: u64,
    pub skipped: u64,
}

pub(crate) struct Run {
    pub span: Span,
    started: Instant,
    finished: bool,
    metrics: Metrics,
    labels: WorkerLabels,
}
impl Run {
    pub fn new(worker: &'static str, task: &'static str, metrics: Metrics) -> Self {
        let run_id = uuid::Uuid::new_v4();
        let labels = WorkerLabels { worker, task };
        metrics.worker_started(&labels);
        Self {
            span: tracing::error_span!("worker", %run_id, worker, task),
            started: Instant::now(),
            finished: false,
            metrics,
            labels,
        }
    }
    pub fn finish(&mut self, report: &Report) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.metrics.worker_finished(
            &self.labels,
            if report.failed > 0 {
                "error"
            } else {
                "success"
            },
            self.started.elapsed().as_secs_f64(),
            Some((report.processed, report.failed, report.skipped)),
        );
        let duration_ms = self.started.elapsed().as_millis() as u64;
        let Report {
            processed,
            failed,
            skipped,
        } = *report;
        if failed > 0 {
            tracing::error!(parent: &self.span, event = "worker.run_finished", outcome = "error",
                processed, failed, skipped, duration_ms);
        } else if processed > 0 {
            tracing::info!(parent: &self.span, event = "worker.run_finished", outcome = "success",
                processed, failed, skipped, duration_ms);
        } else {
            tracing::debug!(parent: &self.span, event = "worker.run_finished", outcome = "success",
                processed, failed, skipped, duration_ms);
        }
    }
    pub fn timed_out(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.metrics.worker_finished(
            &self.labels,
            "timeout",
            self.started.elapsed().as_secs_f64(),
            None,
        );
        tracing::warn!(parent: &self.span, event = "worker.run_finished", outcome = "timeout",
            error_code = "pass_timeout", duration_ms = self.started.elapsed().as_millis() as u64);
    }

    pub fn skipped(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.metrics.worker_finished(
            &self.labels,
            "skipped",
            self.started.elapsed().as_secs_f64(),
            Some((0, 0, 1)),
        );
        tracing::debug!(parent: &self.span, event = "worker.run_finished", outcome = "skipped",
            skipped = 1_u64, duration_ms = self.started.elapsed().as_millis() as u64);
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        if !self.finished {
            self.metrics.worker_finished(
                &self.labels,
                "interrupted",
                self.started.elapsed().as_secs_f64(),
                None,
            );
            tracing::info!(parent: &self.span, event = "worker.run_finished", outcome = "interrupted",
                error_code = "run_dropped",
                duration_ms = self.started.elapsed().as_millis() as u64);
        }
    }
}
