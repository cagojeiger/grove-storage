#![allow(clippy::expect_used)]

use super::*;

fn dummy_s3_spec() -> filegate_infra::S3StorageSpec {
    filegate_infra::S3StorageSpec {
        endpoint: "http://m:9000".to_owned(),
        public_endpoint: "http://m:9000".to_owned(),
        region: "us-east-1".to_owned(),
        bucket: "b".to_owned(),
        force_path_style: true,
        access_key: "ak".to_owned(),
        secret_key: filegate_core::SecretString::from("sk".to_owned()),
    }
}

#[test]
fn spool_root_targets_root_for_fs_and_temp_dir_for_s3() {
    let fs = StorageBackend::Fs {
        root: std::path::PathBuf::from("/data/x"),
    };
    assert_eq!(spool_root(&fs), std::path::PathBuf::from("/data/x"));
    // s3 중계는 OS 로컬 스풀(임시 디렉토리)을 거친다.
    let s3 = StorageBackend::S3 {
        spec: dummy_s3_spec(),
        force_relay: true,
    };
    assert_eq!(spool_root(&s3), std::env::temp_dir());
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
