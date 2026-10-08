use std::{future::Future, time::Duration};
use tokio_util::sync::CancellationToken;

const PASS_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Job {
    Observe,
    NativeComplete,
    NativeCleanup,
    S3Complete,
    S3Expire,
    S3Cleanup,
    Reclaim,
    ReclaimCleanup,
    Purge,
    ReadLeases,
    PruneLeases,
    PruneFiles,
    Snapshot,
}

const JOBS: [Job; 13] = [
    Job::Observe,
    Job::NativeComplete,
    Job::NativeCleanup,
    Job::S3Complete,
    Job::S3Expire,
    Job::S3Cleanup,
    Job::Reclaim,
    Job::ReclaimCleanup,
    Job::Purge,
    Job::ReadLeases,
    Job::PruneLeases,
    Job::PruneFiles,
    Job::Snapshot,
];

#[derive(Default)]
pub(super) struct Schedule {
    remaining: &'static [Job],
}

impl Schedule {
    pub async fn run<F: Future<Output = ()>>(&mut self, mut job: impl FnMut(Job) -> F) {
        for _ in JOBS {
            if let Some((selected, remaining)) =
                self.remaining.split_first().or_else(|| JOBS.split_first())
            {
                // Advance before awaiting so a cancelled job cannot monopolize every pass.
                self.remaining = remaining;
                job(*selected).await;
            }
        }
    }
}

pub(super) async fn bounded_pass<F: Future>(
    shutdown: &CancellationToken,
    pass: F,
) -> Option<Result<F::Output, tokio::time::error::Elapsed>> {
    tokio::select! {
        biased;
        () = shutdown.cancelled() => None,
        result = tokio::time::timeout(PASS_TIMEOUT, pass) => Some(result),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn slow_jobs_cannot_starve_other_kinds_across_passes() {
        let mut schedule = Schedule::default();
        let shutdown = CancellationToken::new();
        let mut calls = Vec::new();
        for expected in JOBS {
            let started = tokio::time::Instant::now();
            let result = bounded_pass(
                &shutdown,
                schedule.run(|job| {
                    calls.push(job);
                    std::future::pending::<()>()
                }),
            )
            .await;
            assert!(matches!(result, Some(Err(_))));
            assert_eq!(started.elapsed(), PASS_TIMEOUT);
            assert_eq!(calls.last(), Some(&expected));
        }
        assert_eq!(calls, JOBS);
        calls.clear();
        let result = bounded_pass(
            &shutdown,
            schedule.run(|job| {
                calls.push(job);
                std::future::ready(())
            }),
        )
        .await;
        assert!(matches!(result, Some(Ok(()))));
        assert_eq!(calls, JOBS);
    }

    #[tokio::test(start_paused = true)]
    async fn pass_timeout_includes_work_before_the_schedule() {
        let mut schedule = Schedule::default();
        let shutdown = CancellationToken::new();
        let mut calls = 0;
        let started = tokio::time::Instant::now();
        let result = bounded_pass(&shutdown, async {
            tokio::time::sleep(PASS_TIMEOUT + Duration::from_secs(1)).await;
            schedule
                .run(|_| {
                    calls += 1;
                    std::future::ready(())
                })
                .await;
        })
        .await;
        assert!(matches!(result, Some(Err(_))));
        assert_eq!(started.elapsed(), PASS_TIMEOUT);
        assert_eq!(calls, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_resumes_after_the_interrupted_not_the_completed_job() {
        let mut schedule = Schedule::default();
        let shutdown = CancellationToken::new();
        let mut calls = Vec::new();
        let result = bounded_pass(
            &shutdown,
            schedule.run(|job| {
                calls.push(job);
                async move {
                    if job == Job::Observe {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                }
            }),
        )
        .await;
        assert!(matches!(result, Some(Err(_))));
        assert_eq!(calls, [Job::Observe, Job::NativeComplete]);
        calls.clear();
        schedule
            .run(|job| {
                calls.push(job);
                std::future::ready(())
            })
            .await;
        assert_eq!(
            calls,
            JOBS.iter()
                .cycle()
                .skip(2)
                .take(JOBS.len())
                .copied()
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_cancels_inflight_work_and_prevents_later_jobs() {
        let shutdown = CancellationToken::new();
        let stopping = shutdown.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            stopping.cancel();
        });
        let mut schedule = Schedule::default();
        let mut calls = Vec::new();
        let started = tokio::time::Instant::now();
        let result = bounded_pass(
            &shutdown,
            schedule.run(|job| {
                calls.push(job);
                std::future::pending::<()>()
            }),
        )
        .await;
        assert!(result.is_none());
        assert_eq!(started.elapsed(), Duration::from_secs(3));
        assert_eq!(calls, [Job::Observe]);
        let mut polled = false;
        assert!(
            bounded_pass(&shutdown, async {
                polled = true;
            })
            .await
            .is_none()
        );
        assert!(!polled);
    }
}
