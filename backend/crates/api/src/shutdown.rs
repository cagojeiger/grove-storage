//! One deadline covers HTTP, all background tasks, and database pool closure.
use std::{future::Future, io, time::Duration};
use tokio::task::JoinSet;

pub const GRACE_PERIOD: Duration = Duration::from_secs(20);

pub async fn drain<F: Future<Output = ()>>(
    server: impl Future<Output = io::Result<()>>,
    mut workers: JoinSet<()>,
    close_pool: impl FnOnce() -> F,
    grace: Duration,
) -> anyhow::Result<()> {
    let result = tokio::time::timeout(grace, async {
        let (server_result, ()) = tokio::join!(server, async {
            while let Some(result) = workers.join_next().await {
                if let Err(error) = result {
                    tracing::warn!(event = "background.join_failed", %error);
                }
            }
        });
        close_pool().await;
        server_result
    })
    .await;
    match result {
        Ok(result) => Ok(result?),
        Err(_) => {
            workers.shutdown().await;
            tracing::error!(event = "shutdown.timed_out", grace_secs = grace.as_secs());
            anyhow::bail!(
                "shutdown exceeded its {} second grace period",
                grace.as_secs_f64()
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_util::sync::CancellationToken;

    const TEST_GRACE: Duration = Duration::from_millis(50);

    fn workers(task: impl Future<Output = ()> + Send + 'static) -> JoinSet<()> {
        let mut workers = JoinSet::new();
        workers.spawn(task);
        workers
    }

    #[tokio::test]
    async fn normal_shutdown_drains_before_closing_pool() {
        let done = Arc::new(AtomicBool::new(false));
        let worker_done = done.clone();
        drain(
            async { Ok(()) },
            workers(async move {
                worker_done.store(true, Ordering::SeqCst);
            }),
            || async {
                assert!(done.load(Ordering::SeqCst));
            },
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn stuck_worker_is_aborted_at_the_deadline() {
        let mut workers = JoinSet::new();
        let first = workers.spawn(std::future::pending::<()>());
        let second = workers.spawn(std::future::pending::<()>());
        let result = drain(async { Ok(()) }, workers, || async {}, TEST_GRACE).await;
        assert!(result.is_err());
        tokio::task::yield_now().await;
        assert!(first.is_finished());
        assert!(second.is_finished());
    }

    #[tokio::test(start_paused = true)]
    async fn pool_close_uses_the_same_deadline() {
        assert!(
            drain(
                async { Ok(()) },
                workers(async {}),
                std::future::pending,
                TEST_GRACE,
            )
            .await
            .is_err()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn worker_and_pool_share_one_total_budget() {
        let start = tokio::time::Instant::now();
        let result = drain(
            async { Ok(()) },
            workers(async {
                tokio::time::sleep(Duration::from_secs(15)).await;
            }),
            || async {
                tokio::time::sleep(Duration::from_secs(6)).await;
            },
            Duration::from_secs(20),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(start.elapsed(), Duration::from_secs(20));
    }

    #[tokio::test]
    async fn server_error_still_closes_the_pool() {
        let closed = AtomicBool::new(false);
        let result = drain(
            async { Err(io::Error::other("server failed")) },
            workers(async {}),
            || async {
                closed.store(true, Ordering::SeqCst);
            },
            Duration::from_secs(1),
        )
        .await;
        assert!(result.is_err());
        assert!(closed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn stalled_http_body_cannot_keep_shutdown_waiting_forever() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let app = axum::Router::new().route(
            "/",
            axum::routing::get(|| async {
                axum::body::Body::from_stream(futures_util::stream::pending::<
                    Result<Vec<u8>, io::Error>,
                >())
            }),
        );
        let server =
            axum::serve(listener, app).with_graceful_shutdown(cancel.clone().cancelled_owned());
        let serving = tokio::spawn(server.into_future());
        let mut connection = tokio::net::TcpStream::connect(addr).await.unwrap();
        connection
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut response = [0; 1024];
        assert!(
            tokio::time::timeout(Duration::from_secs(1), connection.read(&mut response))
                .await
                .unwrap()
                .unwrap()
                > 0
        );
        cancel.cancel();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            drain(
                async { serving.await.map_err(io::Error::other)? },
                workers(async {}),
                || async {},
                TEST_GRACE,
            ),
        )
        .await
        .unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn sqlx_pool_stays_open_until_requests_and_worker_finish() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        let worker_pool = pool.clone();
        drain(
            async {
                assert!(!pool.is_closed());
                Ok(())
            },
            workers(async move {
                assert!(!worker_pool.is_closed());
            }),
            || pool.close(),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert!(pool.is_closed());
    }

    #[tokio::test(start_paused = true)]
    async fn all_workers_finish_before_pool_close() {
        let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut workers = JoinSet::new();
        for delay in [3, 7] {
            let completed = completed.clone();
            workers.spawn(async move {
                tokio::time::sleep(Duration::from_secs(delay)).await;
                completed.fetch_add(1, Ordering::SeqCst);
            });
        }
        let start = tokio::time::Instant::now();
        drain(
            async { Ok(()) },
            workers,
            || async {
                assert_eq!(completed.load(Ordering::SeqCst), 2);
            },
            Duration::from_secs(20),
        )
        .await
        .unwrap();
        assert_eq!(start.elapsed(), Duration::from_secs(7));
    }
}
