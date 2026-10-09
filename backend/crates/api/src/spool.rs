//! Shared Native/S3 transfer admission, bounded stream receive, and measurement.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use futures_util::StreamExt as _;
use md5::{Digest as _, Md5};
use sha2::Sha256;
use tokio::io::AsyncWriteExt as _;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_BYTES_PER_SECOND: u64 = 64 * 1024;
const PROGRESS_WINDOW: Duration = Duration::from_secs(30);
const BUDGET_UNIT: u64 = 1024 * 1024;

pub const SPOOL_CONCURRENCY_LIMIT: usize = 16;

pub struct SpoolBudget {
    slots: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
}

pub struct SpoolPermit {
    _slot: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}

impl SpoolBudget {
    pub fn new(max_bytes: u64) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(SPOOL_CONCURRENCY_LIMIT)),
            bytes: Arc::new(Semaphore::new((max_bytes / BUDGET_UNIT) as usize)),
        }
    }

    /// Round reservations up, capacity down; never queue requests with partial claims.
    pub fn try_acquire(&self, declared_size: i64) -> Option<SpoolPermit> {
        let units = u32::try_from(u64::try_from(declared_size).ok()?.div_ceil(BUDGET_UNIT)).ok()?;
        let slot = self.slots.clone().try_acquire_owned().ok()?;
        let bytes = self.bytes.clone().try_acquire_many_owned(units).ok()?;
        Some(SpoolPermit {
            _slot: slot,
            _bytes: bytes,
        })
    }
}
/// 스트림 버퍼 크기 — 다운로드 재청크와 업로드 스풀 쓰기가 공유한다.
/// 기본 4KiB로 두면 GiB급 전송이 수십만 번의 블로킹 풀 왕복이 된다.
pub const STREAM_BUF_SIZE: usize = 256 * 1024;

/// Request spools are local disposable buffers, independent of storage placement.
pub fn spool_root() -> std::path::PathBuf {
    std::env::temp_dir().join("grove-storage-spool")
}

/// 스풀 실측 결과. S3 요청은 SHA256·CRC32도 실측하며 네이티브 중계는 MD5를 쓴다.
pub struct Measured {
    pub written: i64,
    pub md5_hex: String,
    pub sha256_hex: Option<String>,
    pub crc32: Option<u32>,
}

/// 스풀 실패의 종류 — 호출자가 각자의 표면 에러(ApiError·S3 XML)로 번역한다.
/// 이 프리미티브는 자기 실패 시 임시 파일을 지운다 (abort_write는 멱등이라
/// 호출자가 이중 abort해도 무해하다). 미달(written < declared) 검사는
/// 호출자 몫이다 — 표면마다 에러 코드가 다르므로.
pub enum SpoolError {
    Idle,
    TooSlow,
    Deadline,
    Aborted,
    TooLarge,
    Io(std::io::Error),
}

/// body를 writer에 쓰며 크기·MD5(+선택 SHA256·CRC32)를 실측하고, 선언 크기를 넘는
/// 순간 끊는다. 유휴·단절·초과·IO 실패는 임시 파일을 지우고 에러로 돌아간다.
pub async fn spool_to_temp(
    body: Body,
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    temp_path: &Path,
    declared_size: i64,
    want_s3_checksums: bool,
) -> Result<Measured, SpoolError> {
    spool_to_temp_with_limits(
        body,
        writer,
        temp_path,
        declared_size,
        want_s3_checksums,
        StreamLimits {
            idle: STREAM_IDLE_TIMEOUT,
            window: PROGRESS_WINDOW,
            min_bytes_per_second: MIN_BYTES_PER_SECOND,
        },
    )
    .await
}

struct StreamLimits {
    idle: Duration,
    window: Duration,
    min_bytes_per_second: u64,
}

async fn spool_to_temp_with_limits(
    body: Body,
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    temp_path: &Path,
    declared_size: i64,
    want_s3_checksums: bool,
    limits: StreamLimits,
) -> Result<Measured, SpoolError> {
    let mut md5 = Md5::new();
    let mut sha256 = want_s3_checksums.then(Sha256::new);
    let mut crc32 = want_s3_checksums.then(crc32fast::Hasher::new);
    let mut written: i64 = 0;
    let mut stream = body.into_data_stream();
    let start = tokio::time::Instant::now();
    let deadline = start
        + limits.window
        + Duration::from_secs(
            declared_size
                .max(0)
                .cast_unsigned()
                .div_ceil(limits.min_bytes_per_second),
        );
    let mut idle_deadline = start + limits.idle;
    let mut progress = tokio::time::interval_at(start + limits.window, limits.window);
    progress.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut window_start = start;
    let mut window_written = 0;
    loop {
        let chunk = tokio::select! {
            biased;
            _ = tokio::time::sleep_until(deadline) => {
                abort_spool(temp_path).await;
                return Err(SpoolError::Deadline);
            }
            _ = tokio::time::sleep_until(idle_deadline) => {
                abort_spool(temp_path).await;
                return Err(SpoolError::Idle);
            }
            _ = progress.tick() => {
                let now = tokio::time::Instant::now();
                let required = u128::from(limits.min_bytes_per_second) * (now - window_start).as_nanos();
                let received = u128::from((written - window_written).cast_unsigned()) * 1_000_000_000;
                if received < required {
                    abort_spool(temp_path).await;
                    return Err(SpoolError::TooSlow);
                }
                window_start = now;
                window_written = written;
                continue;
            }
            chunk = stream.next() => match chunk {
                None => break,
                Some(Err(_)) => {
                    abort_spool(temp_path).await;
                    return Err(SpoolError::Aborted);
                }
                Some(Ok(chunk)) => chunk,
            },
        };
        if !chunk.is_empty() {
            idle_deadline = tokio::time::Instant::now() + limits.idle;
        }
        written += chunk.len() as i64;
        if written > declared_size {
            abort_spool(temp_path).await;
            return Err(SpoolError::TooLarge);
        }
        md5.update(&chunk);
        if let Some(crc) = crc32.as_mut() {
            crc.update(&chunk);
        }
        if let Some(sha) = sha256.as_mut() {
            use sha2::Digest as _;
            sha.update(&chunk);
        }
        match tokio::time::timeout_at(deadline, writer.write_all(&chunk)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                abort_spool(temp_path).await;
                return Err(SpoolError::Io(error));
            }
            Err(_) => {
                abort_spool(temp_path).await;
                return Err(SpoolError::Deadline);
            }
        }
    }
    Ok(Measured {
        written,
        md5_hex: hex::encode(md5.finalize()),
        crc32: crc32.map(crc32fast::Hasher::finalize),
        sha256_hex: sha256.map(|sha| {
            use sha2::Digest as _;
            hex::encode(sha.finalize())
        }),
    })
}

async fn abort_spool(temp_path: &Path) {
    grove_infra::temp_spool::abort_write(temp_path).await;
}

#[cfg(test)]
mod tests;
