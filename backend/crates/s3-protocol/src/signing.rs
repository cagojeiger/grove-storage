//! Pure SigV4 signing and raw-query canonicalization.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    // Hmac::new_from_slice는 임의 길이 키를 받아 InvalidLength가 나지 않는다.
    // 그래도 unwrap/expect는 워크스페이스 린트가 막으므로 let-else로 받는다 —
    // 이 분기는 도달 불가다. 설령 도달해도 결과는 실제 서명과 다른 값이라
    // 상수시간 비교에서 불일치(403)로 닫힌다.
    let Ok(mut mac) = <HmacSha256 as Mac>::new_from_slice(key) else {
        return Vec::new();
    };
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// SigV4 서명 계산 — secret + scope(date/region) + string_to_sign → hex 서명.
/// service는 s3, terminator는 aws4_request로 고정이다 (authenticate가 scope를
/// 이미 검증한 뒤 부른다). 순수 함수라 알려진 답 벡터로 테스트한다.
pub fn sign(secret: &str, scope_date: &str, region: &str, string_to_sign: &str) -> String {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), scope_date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()))
}

/// canonical query — 키 정렬. X-Amz-Signature는 제외한다: 어느 서명 모드든
/// 서명 자신은 canonical에 들어가지 않는다(presigned의 핵심 규칙). 받은
/// percent-encoding을 그대로 보존한다 — 서명한 바이트와 같아야 하므로.
pub fn canonicalize_query(query: &str) -> String {
    let mut pairs: Vec<(&str, &str)> = query
        .split('&')
        .filter(|s| !s.is_empty())
        .map(|p| p.split_once('=').unwrap_or((p, "")))
        .filter(|(k, _)| *k != "X-Amz-Signature")
        .collect();
    pairs.sort_unstable();
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}
