use grove_s3_protocol::auth::*;

#[test]
fn expiry_is_bounded_to_one_second_through_seven_days() {
    for value in ["1", "900", "604800"] {
        assert!(presigned_expiry(value).is_ok());
    }
    for value in ["0", "-1", "+1", "604801", "9223372036854775808", "", "1.0"] {
        assert!(presigned_expiry(value).is_err(), "{value}");
    }
}

#[test]
fn scope_has_exactly_five_nonempty_components() {
    assert!(credential_scope("key/20260922/auto/s3/aws4_request").is_ok());
    for value in [
        "/20260922/auto/s3/aws4_request",
        "key//auto/s3/aws4_request",
        "key/20260922//s3/aws4_request",
        "key/20260922/auto/s3/aws4_request/extra",
        "key/20260922/auto/ec2/aws4_request",
        "key/202609/auto/s3/aws4_request",
    ] {
        assert!(credential_scope(value).is_err(), "{value}");
    }
}

#[test]
fn signed_headers_are_ordered_unique_and_include_host() {
    for value in ["host", "content-type;host;x-amz-date"] {
        assert!(signed_headers(value).is_ok());
    }
    for value in [
        "",
        "x-amz-date",
        "Host",
        "host;host",
        "x-amz-date;host",
        "host;",
        "host;x amz",
    ] {
        assert!(signed_headers(value).is_err(), "{value}");
    }
}

#[test]
fn signed_payload_detects_byte_changes_and_explicit_unsigned_mode() {
    let hash = grove_s3_protocol::signing::sha256_hex(b"body");
    assert!(valid_payload_hash(&hash));
    assert!(payload_matches(&hash, b"body"));
    assert!(!payload_matches(&hash, b"body\n"));
    assert!(payload_matches("UNSIGNED-PAYLOAD", b"body\n"));
    assert!(!valid_payload_hash("anything"));
    assert!(!payload_matches("anything", b"body"));
}

#[test]
fn header_whitespace_is_collapsed() {
    assert_eq!(canonical_header_value("  a \t  b  "), "a b");
}
