//! SigV4 header·query 서명을 검증해 client_id를 반환한다.
//! 원본 인코딩으로 canonical request를 구성하고 암호화된 secret으로 서명을 대조한다.

use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use filegate_core::ExposeSecret as _;
use filegate_db::s3_registry as s3reg;
use grove_s3_protocol::auth::{
    canonical_header_value, credential_scope, presigned_expiry, signed_headers, valid_payload_hash,
};
use grove_s3_protocol::signing::{canonicalize_query, sha256_hex, sign};
use subtle::ConstantTimeEq as _;

use super::xml::{access_denied, xml_error, xml_internal};
use super::{header_str, query_value};
use crate::routes::AppState;

/// SigV4 요청 시각의 허용 스큐 (AWS 관례 ±15분). presigned는 여기에 더해
/// X-Amz-Expires 창 안이어야 한다.
const MAX_CLOCK_SKEW_SECS: i64 = 15 * 60;

/// 검증에 필요한 재료 — 두 서명 모드가 같은 형태로 모은다. 여기까지 오면
/// 이후 조립·재계산은 모드와 무관하다.
struct SigV4 {
    access_key: String,
    scope_date: String,
    region: String,
    service: String,
    terminator: String,
    signed_headers: Vec<String>,
    signature: String,
    /// string-to-sign에 들어갈 시각 (header: x-amz-date, query: X-Amz-Date).
    amz_date: String,
    /// canonical request의 payload hash (header: x-amz-content-sha256,
    /// query(presigned): 언제나 UNSIGNED-PAYLOAD).
    payload_hash: String,
    /// canonical query — 받은 인코딩 보존, X-Amz-Signature 제외.
    canonical_query: String,
}

pub(super) struct Authenticated {
    pub client_id: String,
    pub payload_hash: String,
}

/// `AWS4-HMAC-SHA256 Credential=AK/date/region/s3/aws4_request,
///  SignedHeaders=h1;h2, Signature=hex`
struct ParsedAuth {
    access_key: String,
    scope_date: String,
    region: String,
    service: String,
    terminator: String,
    signed_headers: Vec<String>,
    signature: String,
}

fn parse_auth(auth: &str) -> Option<ParsedAuth> {
    let rest = auth.strip_prefix("AWS4-HMAC-SHA256 ")?;
    let mut credential = None;
    let mut signed = None;
    let mut signature = None;
    for part in rest.split(',') {
        let (name, value) = part.trim().split_once('=')?;
        match name {
            "Credential" if credential.is_none() => credential = Some(value),
            "SignedHeaders" => {
                if signed.is_some() {
                    return None;
                }
                signed = Some(signed_headers(value).ok()?)
            }
            "Signature" if signature.is_none() => signature = Some(value.to_owned()),
            _ => return None,
        }
    }
    let scope = credential_scope(credential?).ok()?;
    Some(ParsedAuth {
        access_key: scope.access_key.to_owned(),
        scope_date: scope.date.to_owned(),
        region: scope.region.to_owned(),
        service: "s3".to_owned(),
        terminator: "aws4_request".to_owned(),
        signed_headers: signed?,
        signature: signature?,
    })
}

/// header-signed 재료 — Authorization 헤더 + x-amz-* 헤더에서.
// Err=Response는 s3 표면의 관용구(authenticate와 같음) — sync fn이라만 lint 대상.
#[allow(clippy::result_large_err)]
fn from_header(uri: &Uri, headers: &HeaderMap) -> Result<SigV4, Response> {
    let auth = header_str(headers, "authorization")
        .ok_or_else(|| access_denied("missing authorization"))?;
    let parsed = parse_auth(auth).ok_or_else(|| access_denied("malformed authorization"))?;

    let amz_date =
        header_str(headers, "x-amz-date").ok_or_else(|| access_denied("missing x-amz-date"))?;
    if amz_date.get(..8) != Some(parsed.scope_date.as_str()) {
        return Err(access_denied(
            "x-amz-date does not match the credential scope",
        ));
    }
    let request_time = chrono::NaiveDateTime::parse_from_str(amz_date, "%Y%m%dT%H%M%SZ")
        .map_err(|_| access_denied("malformed x-amz-date"))?
        .and_utc();
    if (chrono::Utc::now() - request_time).num_seconds().abs() > MAX_CLOCK_SKEW_SECS {
        return Err(xml_error(
            StatusCode::FORBIDDEN,
            "RequestTimeTooSkewed",
            "the difference between the request time and the server time is too large",
        ));
    }

    let payload_hash = header_str(headers, "x-amz-content-sha256")
        .ok_or_else(|| access_denied("missing x-amz-content-sha256"))?;
    if payload_hash.starts_with("STREAMING-") {
        // aws-chunked 스트리밍 서명은 청크 디코딩을 요구한다 — 보류 (spec 03).
        // boto3/봇오코어는 HTTP에서 실해시를 보낸다 (실측).
        return Err(xml_error(
            StatusCode::NOT_IMPLEMENTED,
            "NotImplemented",
            "streaming payload signatures are not supported",
        ));
    }

    if !valid_payload_hash(payload_hash) {
        return Err(access_denied("invalid x-amz-content-sha256"));
    }

    Ok(SigV4 {
        access_key: parsed.access_key,
        scope_date: parsed.scope_date,
        region: parsed.region,
        service: parsed.service,
        terminator: parsed.terminator,
        signed_headers: parsed.signed_headers,
        signature: parsed.signature,
        amz_date: amz_date.to_owned(),
        payload_hash: payload_hash.to_owned(),
        canonical_query: uri.query().map(canonicalize_query).unwrap_or_default(),
    })
}

/// query-signed(presigned) 재료 — 서명·자격이 쿼리스트링에 있다. payload는
/// UNSIGNED-PAYLOAD로 고정, 만료는 X-Amz-Expires 창으로 검사한다. 이게 서비스가
/// 자기 S3 SDK의 `generate_presigned_url`을 filegate에 그대로 겨누는 경로다.
#[allow(clippy::result_large_err)]
fn from_query(uri: &Uri) -> Result<SigV4, Response> {
    let query = uri.query().unwrap_or_default();
    for key in [
        "X-Amz-Algorithm",
        "X-Amz-Credential",
        "X-Amz-Date",
        "X-Amz-Expires",
        "X-Amz-SignedHeaders",
        "X-Amz-Signature",
    ] {
        let count = query
            .split('&')
            .filter(|pair| {
                let name = pair.split('=').next().unwrap_or_default();
                percent_encoding::percent_decode_str(name).decode_utf8_lossy() == key
            })
            .count();
        if count != 1 {
            return Err(access_denied("missing or duplicate signature parameter"));
        }
    }
    let algorithm = query_value(query, "X-Amz-Algorithm")
        .ok_or_else(|| access_denied("missing X-Amz-Algorithm"))?;
    if algorithm != "AWS4-HMAC-SHA256" {
        return Err(access_denied("unsupported signing algorithm"));
    }

    // X-Amz-Credential은 `/`가 %2F로 인코딩돼 온다 — scope 파싱용으로만 디코딩.
    let credential = query_value(query, "X-Amz-Credential")
        .ok_or_else(|| access_denied("missing X-Amz-Credential"))?
        .replace("%2F", "/")
        .replace("%2f", "/");
    let scope = credential_scope(&credential).map_err(access_denied)?;
    let access_key = scope.access_key.to_owned();
    let scope_date = scope.date.to_owned();
    let region = scope.region.to_owned();
    let service = "s3".to_owned();
    let terminator = "aws4_request".to_owned();

    let amz_date =
        query_value(query, "X-Amz-Date").ok_or_else(|| access_denied("missing X-Amz-Date"))?;
    if amz_date.get(..8) != Some(scope_date.as_str()) {
        return Err(access_denied(
            "X-Amz-Date does not match the credential scope",
        ));
    }
    let request_time = chrono::NaiveDateTime::parse_from_str(amz_date, "%Y%m%dT%H%M%SZ")
        .map_err(|_| access_denied("malformed X-Amz-Date"))?
        .and_utc();
    let expires = presigned_expiry(query_value(query, "X-Amz-Expires").unwrap_or_default())
        .map_err(access_denied)?;
    let elapsed = (chrono::Utc::now() - request_time).num_seconds();
    if elapsed < -MAX_CLOCK_SKEW_SECS {
        return Err(access_denied("the presigned url is not yet valid"));
    }
    if elapsed > expires {
        return Err(xml_error(
            StatusCode::FORBIDDEN,
            "AccessDenied",
            "the presigned url has expired",
        ));
    }

    let signed_header_value = query_value(query, "X-Amz-SignedHeaders")
        .ok_or_else(|| access_denied("missing X-Amz-SignedHeaders"))?
        .replace("%3B", ";")
        .replace("%3b", ";");
    let signed_headers = signed_headers(&signed_header_value).map_err(access_denied)?;
    let signature = query_value(query, "X-Amz-Signature")
        .ok_or_else(|| access_denied("missing X-Amz-Signature"))?
        .to_owned();

    Ok(SigV4 {
        access_key,
        scope_date,
        region,
        service,
        terminator,
        signed_headers,
        signature,
        amz_date: amz_date.to_owned(),
        payload_hash: "UNSIGNED-PAYLOAD".to_owned(),
        canonical_query: canonicalize_query(query),
    })
}

/// SigV4 검증 → client_id. 실패는 완성된 XML 403이다.
///
/// canonical request의 URI는 요청 라인의 percent-encoded 경로 그대로다 —
/// 클라이언트가 서명한 바이트와 같아야 하므로 디코딩하지 않는다 (실측:
/// 유니코드 키는 인코딩된 채 도착).
pub(super) async fn authenticate(
    state: &AppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<Authenticated, Response> {
    for name in [
        "host",
        "authorization",
        "x-amz-date",
        "x-amz-content-sha256",
    ] {
        if headers.get_all(name).iter().count() > 1 {
            return Err(access_denied("duplicate authentication header"));
        }
    }
    // 서명 위치로 모드를 가른다: Authorization 헤더면 header-signed,
    // 쿼리에 X-Amz-Signature가 있으면 presigned.
    let sig = if header_str(headers, "authorization").is_some() {
        from_header(uri, headers)?
    } else if uri.query().is_some_and(|q| q.contains("X-Amz-Signature=")) {
        from_query(uri)?
    } else {
        return Err(access_denied("missing authorization"));
    };

    if sig.signature.len() != 64 || !sig.signature.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(access_denied("invalid signature encoding"));
    }
    if let Some(hash) = header_str(headers, "x-amz-content-sha256")
        && !valid_payload_hash(hash)
    {
        return Err(access_denied("invalid x-amz-content-sha256"));
    }
    for name in headers
        .keys()
        .filter(|name| name.as_str().starts_with("x-amz-"))
    {
        // Header-signed payload hash is already part of the canonical request.
        if name == "x-amz-content-sha256" && headers.contains_key("authorization") {
            continue;
        }
        if !sig
            .signed_headers
            .iter()
            .any(|signed| signed == name.as_str())
        {
            return Err(access_denied("x-amz header must be signed"));
        }
    }

    if sig.service != "s3" || sig.terminator != "aws4_request" {
        return Err(access_denied(
            "credential scope must be <date>/<region>/s3/aws4_request",
        ));
    }

    let credential = s3reg::get_credential(&state.pool, &sig.access_key)
        .await
        .map_err(|e| xml_internal("credential lookup", e))?
        .ok_or_else(|| {
            xml_error(
                StatusCode::FORBIDDEN,
                "InvalidAccessKeyId",
                "the access key id does not exist",
            )
        })?;
    // 암호화 저장된 secret을 복호한다 — storage 벤더 시크릿과 같은 기계.
    // enc_key_id 라벨이 복호 키를 고르므로(active·PREV) 회전이 자연히 커버되고,
    // AAD=access_key_id가 암호문 재배치를 막는다.
    let secret = state
        .crypto
        .decrypt(
            &credential.enc_key_id,
            &sig.access_key,
            &filegate_core::EncryptedSecret {
                ciphertext: credential.secret_ciphertext,
                nonce: credential.secret_nonce,
            },
        )
        .map_err(|e| xml_internal("secret decrypt", e))?;

    // canonical request — SignedHeaders 목록 순서대로 (소문자:trim값).
    let mut canonical_headers = String::new();
    for name in &sig.signed_headers {
        let values = headers
            .get_all(name)
            .iter()
            .map(|value| value.to_str().map(canonical_header_value))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| access_denied("invalid signed header"))?;
        if values.is_empty() {
            return Err(access_denied("signed header absent"));
        }
        canonical_headers.push_str(name);
        canonical_headers.push(':');
        canonical_headers.push_str(&values.join(","));
        canonical_headers.push('\n');
    }
    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method.as_str(),
        uri.path(),
        sig.canonical_query,
        canonical_headers,
        sig.signed_headers.join(";"),
        sig.payload_hash,
    );
    let scope = format!("{}/{}/s3/aws4_request", sig.scope_date, sig.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        sig.amz_date,
        sha256_hex(canonical_request.as_bytes())
    );

    let expected = sign(
        secret.expose_secret(),
        &sig.scope_date,
        &sig.region,
        &string_to_sign,
    );

    // 서명 비교는 상수 시간 (config.rs 연산자 토큰 대조와 같은 프리미티브).
    if bool::from(expected.as_bytes().ct_eq(sig.signature.as_bytes())) {
        return Ok(Authenticated {
            client_id: credential.client_id,
            payload_hash: header_str(headers, "x-amz-content-sha256")
                .unwrap_or(&sig.payload_hash)
                .to_owned(),
        });
    }
    Err(xml_error(
        StatusCode::FORBIDDEN,
        "SignatureDoesNotMatch",
        "the request signature does not match",
    ))
}

#[cfg(test)]
mod tests;
