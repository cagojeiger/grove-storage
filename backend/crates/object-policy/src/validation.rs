//! create 선언 검증의 공유 규칙 — 순수 계약 로직만 모은다. 핸들러는 이
//! 함수들을 호출만 하므로 계약이 핸들러 실행 없이 유닛 테스트된다 (spec 00).
//! 표면마다 재구현하면 한쪽만 상한을 빠뜨리거나(5GiB 우회) 잘못된 메타를
//! 조용히 흘린다.

use crate::multipart::MAX_PARTS;

/// v0 단일 PUT 상한 (spec 00: 5GiB 초과는 multipart 경로를 사용).
/// 회계 합산의 overflow 방어이기도 하다.
pub const MAX_SINGLE_PUT_BYTES: i64 = 5 * 1024 * 1024 * 1024;

/// content-type이 저장 가능한 형태인가 — 인쇄 가능 ASCII, 255자 이하.
/// 헤더 인젝션·경로 오염을 막고, 두 표면이 같은 값만 받게 한다.
pub fn content_type_ok(content_type: &str) -> bool {
    content_type.len() <= 255 && content_type.bytes().all(|b| (0x20..0x7f).contains(&b))
}

/// create의 크기·모드 규칙 (spec 00·02). 반환은 is_multipart — 임계값을
/// 넘으면 multipart다. 위반은 표면 에러 메시지 그대로 돌려준다.
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
        // 크기 상한은 part 수 한계(벤더 10,000)로 정해진다.
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
