use std::future::Future;
use tokio::time::Instant;

struct Observation {
    operation: &'static str,
    started: Instant,
    finished: bool,
}

impl Observation {
    fn finish(&mut self, outcome: &'static str) {
        if self.finished {
            return;
        }
        self.finished = true;
        metrics::counter!("grove_provider_requests_total", "operation" => self.operation,
            "outcome" => outcome)
        .increment(1);
        metrics::histogram!("grove_provider_duration_seconds", "operation" => self.operation)
            .record(self.started.elapsed().as_secs_f64());
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        self.finish("interrupted");
    }
}

pub(super) async fn observe<T, E>(
    operation: &'static str,
    future: impl Future<Output = Result<T, aws_sdk_s3::error::SdkError<E>>>,
) -> Result<T, aws_sdk_s3::error::SdkError<E>> {
    let mut observation = Observation {
        operation,
        started: Instant::now(),
        finished: false,
    };
    let result = future.await;
    observation.finish(match &result {
        Ok(_) => "success",
        Err(error)
            if error
                .raw_response()
                .is_some_and(|response| response.status().as_u16() == 404) =>
        {
            "not_found"
        }
        Err(_) => "error",
    });
    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn provider_errors_and_cancellation_preserve_results_and_elapsed_time() {
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new()
            .set_buckets(&[1.0, 5.0])
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .start_paused(true)
                .build()
                .unwrap()
                .block_on(async {
                    type Response = Result<u8, aws_sdk_s3::error::SdkError<std::io::Error>>;
                    assert_eq!(
                        observe("head", async {
                            Ok::<_, aws_sdk_s3::error::SdkError<std::io::Error>>(7)
                        })
                        .await
                        .unwrap(),
                        7
                    );
                    assert!(
                        observe("head", async {
                            Err::<u8, _>(
                                aws_sdk_s3::error::SdkError::<std::io::Error>::construction_failure(
                                    std::io::Error::other("private-provider-value"),
                                ),
                            )
                        })
                        .await
                        .is_err()
                    );
                    let pending = observe("get", std::future::pending::<Response>());
                    tokio::pin!(pending);
                    assert!(
                        tokio::time::timeout(std::time::Duration::from_secs(5), &mut pending)
                            .await
                            .is_err()
                    );
                });
        });
        let text = handle.render();
        assert!(
            text.contains(
                "grove_provider_requests_total{operation=\"head\",outcome=\"success\"} 1"
            ),
            "{text}"
        );
        assert!(
            text.contains("grove_provider_requests_total{operation=\"head\",outcome=\"error\"} 1"),
            "{text}"
        );
        assert!(
            text.contains(
                "grove_provider_requests_total{operation=\"get\",outcome=\"interrupted\"} 1"
            ),
            "{text}"
        );
        assert!(
            text.contains("grove_provider_duration_seconds_sum{operation=\"get\"} 5"),
            "{text}"
        );
        assert!(!text.contains("private-provider-value"));
    }
}
