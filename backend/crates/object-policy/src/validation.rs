//! Native와 S3 표면이 공유하는 업로드 선언 검증.

use crate::multipart::MAX_PARTS;

/// 5GiB 초과는 multipart 경로를 사용한다.
pub const MAX_SINGLE_PUT_BYTES: i64 = 5 * 1024 * 1024 * 1024;

/// content-type이 저장 가능한 형태인가 — 인쇄 가능 ASCII, 255자 이하.
/// 헤더 인젝션·경로 오염을 막고, 두 표면이 같은 값만 받게 한다.
pub fn content_type_ok(content_type: &str) -> bool {
    content_type.len() <= 255 && content_type.bytes().all(|b| (0x20..0x7f).contains(&b))
}

/// 반환값은 multipart 여부다.
/// 순서가 계약이다: multipart면 상한과 md5-무효를 단일 PUT 상한보다 먼저 본다.
pub fn classify_upload(
    declared_size: i64,
    multipart_threshold: i64,
    part_size: i64,
    has_declared_md5: bool,
) -> Result<bool, &'static str> {
    if declared_size < 0 {
        return Err("declared_size must be >= 0");
    }
    let multipart = declared_size > multipart_threshold;
    if multipart {
        if declared_size > part_size.saturating_mul(i64::from(MAX_PARTS)) {
            return Err("declared_size exceeds the multipart limit");
        }
        // 전체 md5는 multipart의 어떤 모드에서도 실측되지 않는다 (ADR 002) —
        // 받아주면 거짓 계약이라 거부한다.
        if has_declared_md5 {
            return Err(
                "declared_md5 is not accepted for multipart uploads (verification is per part)",
            );
        }
    } else if declared_size > MAX_SINGLE_PUT_BYTES {
        return Err("declared_size exceeds the single-upload limit (5 GiB)");
    }
    Ok(multipart)
}

/// 선언 md5의 형태 — 소문자·대문자 32 hex (commit이 ETag와 대소문자 무시로
/// 대조하므로 대문자도 받는다). 값 일치가 아니라 형태만 본다.
pub fn declared_md5_format_ok(md5: &str) -> bool {
    md5.len() == 32 && md5.bytes().all(|b| b.is_ascii_hexdigit())
}
