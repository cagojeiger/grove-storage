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
}
impl Run {
    pub fn new(worker: &'static str, task: &'static str) -> Self {
        let run_id = uuid::Uuid::new_v4();
        Self {
            span: tracing::error_span!("worker", %run_id, worker, task),
            started: Instant::now(),
            finished: false,
        }
    }
    pub fn finish(&mut self, report: &Report) {
        if self.finished {
            return;
        }
        self.finished = true;
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
        tracing::warn!(parent: &self.span, event = "worker.run_finished", outcome = "timeout",
            error_code = "pass_timeout", duration_ms = self.started.elapsed().as_millis() as u64);
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        if !self.finished {
            tracing::info!(parent: &self.span, event = "worker.run_finished", outcome = "interrupted",
                error_code = "run_dropped",
                duration_ms = self.started.elapsed().as_millis() as u64);
        }
    }
}
