#![allow(clippy::expect_used)]

use super::*;

#[tokio::test(start_paused = true)]
async fn idle_stream_stops_at_the_injected_deadline() {
    let body = Body::from_stream(futures_util::stream::pending::<
        Result<axum::body::Bytes, std::io::Error>,
    >());
    let start = tokio::time::Instant::now();
    let result = spool_to_temp_with_limits(
        body,
        &mut tokio::io::sink(),
        Path::new("unused-idle-path"),
        1,
        false,
        StreamLimits {
            idle: Duration::from_secs(30),
            window: PROGRESS_WINDOW,
            min_bytes_per_second: MIN_BYTES_PER_SECOND,
        },
    )
    .await;
    assert!(matches!(result, Err(SpoolError::Idle)));
    assert_eq!(start.elapsed(), Duration::from_secs(30));
}

#[tokio::test(start_paused = true)]
async fn healthy_stream_progress_resets_the_idle_budget() {
    let stream = futures_util::stream::unfold(0, |index| async move {
        if index == 2 {
            return None;
        }
        tokio::time::sleep(Duration::from_secs(29)).await;
        Some((
            Ok::<_, std::io::Error>(axum::body::Bytes::from(vec![b'x'; 2 * 1024 * 1024])),
            index + 1,
        ))
    });
    let start = tokio::time::Instant::now();
    let result = spool_to_temp_with_limits(
        Body::from_stream(stream),
        &mut tokio::io::sink(),
        Path::new("unused-progress-path"),
        4 * 1024 * 1024,
        false,
        StreamLimits {
            idle: Duration::from_secs(30),
            window: PROGRESS_WINDOW,
            min_bytes_per_second: MIN_BYTES_PER_SECOND,
        },
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(start.elapsed(), Duration::from_secs(58));
}

#[test]
fn spool_root_is_local_and_independent_of_provider() {
    assert_eq!(
        spool_root(),
        std::env::temp_dir().join("grove-storage-spool")
    );
}

#[test]
fn spool_reserves_aggregate_bytes_without_queuing() {
    let budget = SpoolBudget::new(6 * BUDGET_UNIT);
    let permit = budget
        .try_acquire(5 * BUDGET_UNIT as i64)
        .expect("capacity");
    assert!(budget.try_acquire(2 * BUDGET_UNIT as i64).is_none());
    assert_eq!(
        budget.slots.available_permits(),
        SPOOL_CONCURRENCY_LIMIT - 1
    );
    drop(permit);
    assert!(budget.try_acquire(6 * BUDGET_UNIT as i64).is_some());
    assert!(budget.try_acquire(-1).is_none());
    assert!(budget.try_acquire(6 * BUDGET_UNIT as i64 + 1).is_none());
}

#[test]
fn spool_bounds_tiny_transfers_and_rounds_reservations_conservatively() {
    let budget = SpoolBudget::new(32 * BUDGET_UNIT + BUDGET_UNIT - 1);
    let permits: Vec<_> = (0..SPOOL_CONCURRENCY_LIMIT)
        .map(|_| budget.try_acquire(1).expect("capacity"))
        .collect();
    assert!(budget.try_acquire(0).is_none());
    assert_eq!(budget.bytes.available_permits(), 16);
    drop(permits);
    assert_eq!(budget.bytes.available_permits(), 32);
}

#[tokio::test(start_paused = true)]
async fn tiny_trickle_is_rejected_even_when_it_is_not_idle() {
    let stream = futures_util::stream::unfold((), |()| async {
        tokio::time::sleep(Duration::from_secs(29)).await;
        Some((
            Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"x")),
            (),
        ))
    });
    let start = tokio::time::Instant::now();
    let result = spool_to_temp(
        Body::from_stream(stream),
        &mut tokio::io::sink(),
        Path::new("unused-trickle-path"),
        1024 * 1024,
        false,
    )
    .await;
    assert!(matches!(result, Err(SpoolError::TooSlow)));
    assert_eq!(start.elapsed(), PROGRESS_WINDOW);
}

#[tokio::test(start_paused = true)]
async fn progress_accepts_the_exact_minimum_and_rejects_one_byte_less() {
    let minimum = MIN_BYTES_PER_SECOND * PROGRESS_WINDOW.as_secs();
    for size in [minimum, minimum - 1] {
        let stream = futures_util::stream::unfold(false, move |sent| async move {
            if sent {
                tokio::time::sleep(Duration::from_secs(2)).await;
                return None;
            }
            tokio::time::sleep(Duration::from_secs(29)).await;
            Some((
                Ok::<_, std::io::Error>(axum::body::Bytes::from(vec![b'x'; size as usize])),
                true,
            ))
        });
        let result = spool_to_temp(
            Body::from_stream(stream),
            &mut tokio::io::sink(),
            Path::new("unused-rate-boundary-path"),
            size as i64,
            false,
        )
        .await;
        if size == minimum {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(SpoolError::TooSlow)));
        }
    }
}

#[tokio::test(start_paused = true)]
async fn empty_chunks_do_not_extend_the_idle_deadline() {
    let stream = futures_util::stream::unfold((), |()| async {
        tokio::time::sleep(Duration::from_secs(29)).await;
        Some((Ok::<_, std::io::Error>(axum::body::Bytes::new()), ()))
    });
    let start = tokio::time::Instant::now();
    let result = spool_to_temp(
        Body::from_stream(stream),
        &mut tokio::io::sink(),
        Path::new("unused-empty-chunk-path"),
        1024 * 1024,
        false,
    )
    .await;
    assert!(matches!(result, Err(SpoolError::Idle)));
    assert_eq!(start.elapsed(), STREAM_IDLE_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn a_stalled_writer_cannot_outlive_the_absolute_deadline() {
    let (mut writer, _reader) = tokio::io::duplex(1);
    let start = tokio::time::Instant::now();
    let result = spool_to_temp_with_limits(
        Body::from("xx"),
        &mut writer,
        Path::new("unused-writer-path"),
        2,
        false,
        StreamLimits {
            idle: Duration::from_secs(30),
            window: Duration::from_secs(30),
            min_bytes_per_second: 1,
        },
    )
    .await;
    assert!(matches!(result, Err(SpoolError::Deadline)));
    assert_eq!(start.elapsed(), Duration::from_secs(32));
}

#[tokio::test]
async fn cancellation_releases_spool_admission() {
    let budget = Arc::new(SpoolBudget::new(BUDGET_UNIT));
    let admitted = budget.clone();
    let (ready, started) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let _permit = admitted.try_acquire(BUDGET_UNIT as i64).expect("capacity");
        ready.send(()).expect("waiting test");
        std::future::pending::<()>().await;
    });
    started.await.expect("admitted");
    assert!(budget.try_acquire(1).is_none());
    task.abort();
    assert!(task.await.expect_err("cancelled").is_cancelled());
    assert!(budget.try_acquire(BUDGET_UNIT as i64).is_some());
}

#[tokio::test]
async fn checksums_cover_all_stream_chunks() {
    let chunks = futures_util::stream::iter([
        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"1234")),
        Ok(axum::body::Bytes::from_static(b"56789")),
    ]);
    let measured = spool_to_temp(
        Body::from_stream(chunks),
        &mut tokio::io::sink(),
        Path::new("unused-success-path"),
        9,
        true,
    )
    .await
    .ok()
    .expect("measurement succeeds");
    assert_eq!(measured.written, 9);
    assert_eq!(measured.crc32, Some(0xcbf43926));
    assert_eq!(measured.md5_hex, "25f9e794323b453885f5181f1b624d0b");
    assert_eq!(
        measured.sha256_hex,
        Some(grove_s3_protocol::signing::sha256_hex(b"123456789"))
    );
}

#[tokio::test]
async fn native_measurement_keeps_only_md5() {
    let measured = spool_to_temp(
        Body::from("123456789"),
        &mut tokio::io::sink(),
        Path::new("unused-success-path"),
        9,
        false,
    )
    .await
    .ok()
    .expect("measurement succeeds");
    assert_eq!(measured.md5_hex, "25f9e794323b453885f5181f1b624d0b");
    assert_eq!(measured.sha256_hex, None);
    assert_eq!(measured.crc32, None);
}
