#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use super::*;

#[test]
fn authorization_rejects_ambiguous_fields_and_scope() {
    let valid = "AWS4-HMAC-SHA256 Credential=key/20260922/auto/s3/aws4_request, SignedHeaders=host;x-amz-date, Signature=abc";
    assert!(parse_auth(valid).is_some());
    for invalid in [
        format!("{valid}, Credential=key/20260922/auto/s3/aws4_request"),
        format!("{valid}, Signature=abc"),
        valid.replace("aws4_request,", "aws4_request/extra,"),
        valid.replace("host;x-amz-date", "x-amz-date"),
        valid.replace("host;x-amz-date", "x-amz-date;host"),
    ] {
        assert!(parse_auth(&invalid).is_none(), "{invalid}");
    }
}

#[test]
fn query_auth_rejects_expiry_bounds_and_duplicate_fields() {
    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date = &now[..8];
    let valid = format!(
        "/b/k?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=key%2F{date}%2Fauto%2Fs3%2Faws4_request&X-Amz-Date={now}&X-Amz-Expires=900&X-Amz-SignedHeaders=host&X-Amz-Signature=abc"
    );
    for invalid in [
        valid.replace("Expires=900", "Expires=0"),
        valid.replace("Expires=900", "Expires=604801"),
        format!("{valid}&X-Amz-Signature=abc"),
        format!("{valid}&X-Amz-%53ignature=abc"),
        valid.replace("aws4_request&", "aws4_request%2Fextra&"),
    ] {
        assert!(from_query(&invalid.parse().unwrap()).is_err(), "{invalid}");
    }
}

#[test]
fn header_auth_rejects_invalid_payload_hash_before_lookup() {
    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date = &now[..8];
    let mut headers = HeaderMap::new();
    headers.insert("authorization", format!("AWS4-HMAC-SHA256 Credential=key/{date}/auto/s3/aws4_request, SignedHeaders=host;x-amz-date, Signature=abc").parse().unwrap());
    headers.insert("x-amz-date", now.parse().unwrap());
    headers.insert("x-amz-content-sha256", "not-a-hash".parse().unwrap());
    assert!(from_header(&"/b/k".parse().unwrap(), &headers).is_err());
    headers.insert("x-amz-content-sha256", "UNSIGNED-PAYLOAD".parse().unwrap());
    assert!(from_header(&"/b/k".parse().unwrap(), &headers).is_ok());
}

#[test]
fn query_param_reads_the_value_or_none() {
    let q = "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=abc&X-Amz-Expires=900";
    assert_eq!(query_value(q, "X-Amz-Signature"), Some("abc"));
    assert_eq!(query_value(q, "X-Amz-Expires"), Some("900"));
    assert_eq!(query_value(q, "X-Amz-Missing"), None);
}

#[test]
fn from_query_parses_credential_scope_and_defaults_unsigned_payload() {
    // 유효 창 안의 최근 시각으로 만든 presigned 쿼리 (서명값은 형태만).
    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date = &now[..8];
    let uri: Uri = format!(
        "/b/k?X-Amz-Algorithm=AWS4-HMAC-SHA256\
         &X-Amz-Credential=fgak0123456789abcdef%2F{date}%2Fauto%2Fs3%2Faws4_request\
         &X-Amz-Date={now}&X-Amz-Expires=900&X-Amz-SignedHeaders=host&X-Amz-Signature=deadbeef"
    )
    .parse()
    .unwrap();
    let sig = from_query(&uri).expect("valid presigned query parses");
    assert_eq!(sig.access_key, "fgak0123456789abcdef");
    assert_eq!(sig.region, "auto");
    assert_eq!(sig.service, "s3");
    assert_eq!(sig.terminator, "aws4_request");
    assert_eq!(sig.signed_headers, vec!["host".to_owned()]);
    assert_eq!(sig.payload_hash, "UNSIGNED-PAYLOAD");
    assert_eq!(sig.signature, "deadbeef");
}

#[test]
fn from_query_rejects_expired_url() {
    // X-Amz-Date가 만료창 훨씬 전이면 거부된다.
    let uri: Uri = "/b/k?X-Amz-Algorithm=AWS4-HMAC-SHA256\
         &X-Amz-Credential=ak%2F20200101%2Fauto%2Fs3%2Faws4_request\
         &X-Amz-Date=20200101T000000Z&X-Amz-Expires=900&X-Amz-SignedHeaders=host&X-Amz-Signature=x"
        .parse()
        .unwrap();
    assert!(from_query(&uri).is_err());
}
