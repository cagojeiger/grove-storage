use grove_object_policy::multipart::MAX_PARTS;
use grove_object_policy::validation::*;

#[test]
fn error_precedence_preserves_the_existing_api_contract() {
    assert_eq!(
        classify_upload(-1, 64, 64, true),
        Err("declared_size must be >= 0")
    );
    assert_eq!(
        classify_upload(640_001, 64, 64, true),
        Err("declared_size exceeds the multipart limit")
    );
    assert_eq!(
        classify_upload(MAX_SINGLE_PUT_BYTES + 1, 64, MAX_SINGLE_PUT_BYTES, true),
        Err("declared_md5 is not accepted for multipart uploads (verification is per part)")
    );
}

#[test]
fn multipart_limit_multiplication_saturates() {
    assert_eq!(classify_upload(i64::MAX, 64, i64::MAX, false), Ok(true));
}

#[test]
fn content_type_accepts_printable_ascii_and_rejects_the_rest() {
    assert!(content_type_ok("image/png"));
    assert!(content_type_ok("")); // 빈 값은 선언 안 함 — 허용
    assert!(content_type_ok(&"a".repeat(255))); // 경계: 255
    assert!(!content_type_ok(&"a".repeat(256))); // 길이 초과
    assert!(!content_type_ok("text/\u{1f}plain")); // 제어 문자
    assert!(!content_type_ok("text/\u{7f}plain")); // DEL(0x7f)
    assert!(!content_type_ok("이미지/png")); // 비 ASCII
}

#[test]
fn classify_upload_splits_single_put_from_multipart_at_the_threshold() {
    let (thr, ps) = (64, 64); // 임계·part 크기
    // 임계 이하는 단일 PUT(false), 초과는 multipart(true).
    assert_eq!(classify_upload(0, thr, ps, false), Ok(false));
    assert_eq!(classify_upload(thr, thr, ps, false), Ok(false)); // 경계: 같으면 단일
    assert_eq!(classify_upload(thr + 1, thr, ps, false), Ok(true)); // 경계: +1이면 multipart
}

#[test]
fn classify_upload_rejects_negative_and_over_limit_sizes() {
    assert_eq!(
        classify_upload(-1, 64, 64, false),
        Err("declared_size must be >= 0")
    );
    // 단일 PUT 상한 5GiB: 임계가 그보다 커야 이 분기에 닿는다.
    let big = 6 * 1024 * 1024 * 1024;
    assert_eq!(
        classify_upload(MAX_SINGLE_PUT_BYTES, big, 64, false),
        Ok(false)
    );
    assert_eq!(
        classify_upload(MAX_SINGLE_PUT_BYTES + 1, big, 64, false),
        Err("declared_size exceeds the single-upload limit (5 GiB)")
    );
    // multipart 상한 = part_size × 10,000.
    let ps = 64;
    assert_eq!(
        classify_upload(ps * i64::from(MAX_PARTS), 64, ps, false),
        Ok(true)
    );
    assert_eq!(
        classify_upload(ps * i64::from(MAX_PARTS) + 1, 64, ps, false),
        Err("declared_size exceeds the multipart limit")
    );
}

#[test]
fn classify_upload_rejects_declared_md5_on_multipart_only() {
    // 단일 PUT은 md5를 받는다(실측 대조용), multipart는 거부한다.
    assert_eq!(classify_upload(10, 64, 64, true), Ok(false));
    assert_eq!(
        classify_upload(1_000, 64, 64, true),
        Err("declared_md5 is not accepted for multipart uploads (verification is per part)")
    );
}

#[test]
fn declared_md5_format_accepts_32_hex_either_case() {
    assert!(declared_md5_format_ok("0123456789abcdef0123456789abcdef"));
    assert!(declared_md5_format_ok("0123456789ABCDEF0123456789ABCDEF")); // 대문자도 형태로 인정
    assert!(!declared_md5_format_ok("0123456789abcdef0123456789abcde")); // 31자
    assert!(!declared_md5_format_ok("0123456789abcdef0123456789abcdef0")); // 33자
    assert!(!declared_md5_format_ok("0123456789abcdef0123456789abcdeg")); // 비 hex(g)
}
