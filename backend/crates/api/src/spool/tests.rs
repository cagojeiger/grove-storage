#![allow(clippy::expect_used)]

use super::*;

#[test]
fn spool_root_is_local_and_independent_of_provider() {
    assert_eq!(spool_root(), std::env::temp_dir());
}

#[tokio::test]
async fn spool_permit_is_held_until_transfer_finishes() {
    let slots = Arc::new(Semaphore::new(1));
    let permit = acquire_spool_slot(&slots).await.expect("open semaphore");
    assert_eq!(slots.available_permits(), 0);
    drop(permit);
    assert_eq!(slots.available_permits(), 1);
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
