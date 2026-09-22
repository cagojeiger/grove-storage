#![allow(clippy::unwrap_used)]
use grove_domain::*;

#[test]
fn identifiers_preserve_legacy_slug_boundaries() {
    for value in ["a", "1", "r2-primary", &"a".repeat(63)] {
        assert_eq!(StorageId::parse(value).unwrap().as_str(), value);
    }
    for value in ["", "-a", "a-", "A", "a/b", "a b", "한글", &"a".repeat(64)] {
        assert_eq!(StorageId::parse(value), Err(Invalid::StorageId));
    }
}

#[test]
fn endpoint_rejects_credentials_and_ambiguous_components() {
    for value in [
        "https://s3.example.com",
        "http://localhost:9000",
        "https://s3.example.com/prefix",
    ] {
        assert!(Endpoint::parse(value).is_ok());
    }
    for value in [
        "ftp://s3.example.com",
        "https://user:secret@s3.example.com",
        "https://s3.example.com?token=secret",
        "https://s3.example.com#secret",
        " https://s3.example.com",
        "https://s3.ex\nample.com",
        "not-a-url",
    ] {
        let error = Endpoint::parse(value).unwrap_err();
        assert_eq!(error, Invalid::Endpoint);
        assert!(!error.to_string().contains("secret"));
    }
}

#[test]
fn capacity_is_a_baseline_not_an_admission_quota() {
    assert_eq!(Capacity::new(0).unwrap().bytes(), 0);
    assert_eq!(Capacity::new(i64::MAX).unwrap().bytes(), i64::MAX);
    assert_eq!(Capacity::new(-1), Err(Invalid::Capacity));
}

#[test]
fn region_and_bucket_are_validated_before_external_access() {
    let endpoint = Endpoint::parse("https://s3.example.com").unwrap();
    assert_eq!(
        S3Target::new(endpoint.clone(), "".into(), "bucket".into(), false),
        Err(Invalid::Region)
    );
    for bucket in ["", "a/b", "a\\b", "a b"] {
        assert_eq!(
            S3Target::new(endpoint.clone(), "auto".into(), bucket.into(), false),
            Err(Invalid::Bucket)
        );
    }
}
