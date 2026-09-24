use grove_s3_protocol::integrity::{DigestError, is_conditional_header, verify_base64, verify_hex};

#[test]
fn read_if_match_uses_strong_etags_and_wildcard() {
    use grove_s3_protocol::integrity::if_match_matches;
    for value in ["*", "\"abc\"", "\"other\", \"abc\""] {
        assert!(if_match_matches(value, Some("abc")));
    }
    for value in ["W/\"abc\"", "abc", "\"ABC\"", "\"old\""] {
        assert!(!if_match_matches(value, Some("abc")));
    }
}

#[test]
fn validates_standard_base64_and_exact_digest_lengths() {
    assert_eq!(verify_base64("AQIDBA==", &[1, 2, 3, 4]), Ok(()));
    assert_eq!(
        verify_base64("AAAAAA==", &[1, 2, 3, 4]),
        Err(DigestError::BadDigest)
    );
    for invalid in ["not base64", "AQ==", "AQIDBA"] {
        assert_eq!(
            verify_base64(invalid, &[1, 2, 3, 4]),
            Err(DigestError::InvalidDigest)
        );
    }
}

#[test]
fn compares_encoded_md5_and_sha256_with_measured_hex() {
    assert_eq!(
        verify_hex(
            "1B2M2Y8AsgTpgAmY7PhCfg==",
            "d41d8cd98f00b204e9800998ecf8427e"
        ),
        Ok(())
    );
    assert_eq!(
        verify_hex(
            "47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ),
        Ok(())
    );
}

#[test]
fn distinguishes_conditions_from_unconditional_headers() {
    for name in [
        "if-match",
        "if-none-match",
        "if-modified-since",
        "if-unmodified-since",
        "if-range",
        "x-amz-if-match-size",
        "x-amz-if-match-last-modified-time",
    ] {
        assert!(is_conditional_header(name));
    }
    assert!(!is_conditional_header("range"));
    assert!(!is_conditional_header("content-type"));
}
