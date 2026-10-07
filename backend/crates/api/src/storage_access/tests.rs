#![allow(clippy::unwrap_used, clippy::panic)]
use super::*;
use grove_core::{ExposeSecret, SecretString};

fn fixture() -> (Crypto, StorageRow) {
    let crypto = Crypto::new("key", &SecretString::from("x".repeat(32))).unwrap();
    let encrypted = crypto
        .encrypt("home", &SecretString::from("provider-secret".to_owned()))
        .unwrap();
    let row = StorageRow {
        id: "home".into(),
        kind: "s3".into(),
        force_relay: false,
        endpoint: Some("http://internal.invalid".into()),
        public_endpoint: Some("https://public.invalid".into()),
        region: Some("local".into()),
        bucket: Some("objects".into()),
        force_path_style: true,
        access_key: Some("access".into()),
        secret_key_ciphertext: Some(encrypted.ciphertext),
        secret_key_nonce: Some(encrypted.nonce),
        enc_key_id: Some("key".into()),
        capacity_bytes: 100,
    };
    (crypto, row)
}

#[test]
fn resolving_settings_preserves_the_row_and_does_not_probe_the_provider() {
    let (crypto, mut row) = fixture();
    for force_relay in [false, true] {
        row.force_relay = force_relay;
        let before = row.clone();
        let backend = backend_from_row(&crypto, &row).unwrap();
        assert_eq!(row, before);
        assert_eq!(backend.is_relay(), force_relay);
        let StorageBackend { spec, .. } = backend;
        assert_eq!(spec.endpoint, "http://internal.invalid");
        assert_eq!(spec.public_endpoint, "https://public.invalid");
        assert_eq!(spec.region, "local");
        assert_eq!(spec.bucket, "objects");
        assert!(spec.force_path_style);
        assert_eq!(spec.access_key, "access");
        assert_eq!(spec.secret_key.expose_secret(), "provider-secret");
    }
}

#[test]
fn provider_credentials_remain_bound_to_the_storage_id() {
    let (crypto, mut row) = fixture();
    row.id = "other".into();
    assert!(backend_from_row(&crypto, &row).is_err());
}

#[test]
fn incomplete_or_unknown_settings_are_rejected() {
    let (crypto, row) = fixture();
    let mut missing = row.clone();
    missing.secret_key_nonce = None;
    assert!(backend_from_row(&crypto, &missing).is_err());
    let mut missing = row.clone();
    missing.public_endpoint = None;
    assert!(backend_from_row(&crypto, &missing).is_err());
    let mut unknown = row;
    unknown.kind = "unknown".into();
    assert!(backend_from_row(&crypto, &unknown).is_err());
}
