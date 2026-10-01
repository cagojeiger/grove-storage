#![allow(clippy::unwrap_used)]

use filegate_core::{Crypto, EncryptedSecret, ExposeSecret, SecretString};
use grove_management_service::resources::PreparedCredential;

fn root() -> SecretString {
    SecretString::from("credential-test-root-at-least-32-bytes")
}

#[test]
fn prepared_key_matches_issued_secret_and_binds_ciphertext_to_access_id() {
    let crypto = Crypto::new("v1", &root()).unwrap();
    let prepared = PreparedCredential::new(&crypto).unwrap();
    let row = prepared.encrypted();
    let encrypted = EncryptedSecret {
        ciphertext: row.ciphertext.to_vec(),
        nonce: row.nonce.to_vec(),
    };
    assert_eq!(row.enc_key_id, "v1");
    let plaintext = crypto
        .decrypt(row.enc_key_id, row.access_key_id, &encrypted)
        .unwrap();
    assert!(
        crypto
            .decrypt(row.enc_key_id, "another-access-key", &encrypted)
            .is_err()
    );
    let id = row.access_key_id.to_owned();
    let output = prepared.into_output();
    assert_eq!(output.access_key_id, id);
    assert_eq!(output.secret_key, plaintext.expose_secret());
    assert_ne!(encrypted.ciphertext, output.secret_key.as_bytes());
    let serialized = serde_json::to_value(output).unwrap();
    assert_eq!(serialized.as_object().unwrap().len(), 2);
    assert!(serialized.get("access_key_id").is_some());
    assert!(serialized.get("secret_key").is_some());
}

#[test]
fn each_preparation_uses_fresh_identifiers_and_secrets_in_the_existing_format() {
    let crypto = Crypto::new("v1", &root()).unwrap();
    let first = PreparedCredential::new(&crypto).unwrap().into_output();
    let second = PreparedCredential::new(&crypto).unwrap().into_output();
    assert_ne!(first.access_key_id, second.access_key_id);
    assert_ne!(first.secret_key, second.secret_key);
    for output in [first, second] {
        let suffix = output.access_key_id.strip_prefix("fgak").unwrap();
        assert_eq!(suffix.len(), 20);
        assert!(suffix.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(output.secret_key.len(), 64);
        assert!(
            output
                .secret_key
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        );
    }
}

#[test]
fn preparation_uses_the_active_encryption_key_during_rotation() {
    let crypto = Crypto::new("v2", &root())
        .unwrap()
        .with_prev("v1", &root())
        .unwrap();
    let prepared = PreparedCredential::new(&crypto).unwrap();
    let row = prepared.encrypted();
    let encrypted = EncryptedSecret {
        ciphertext: row.ciphertext.to_vec(),
        nonce: row.nonce.to_vec(),
    };
    assert_eq!(row.enc_key_id, "v2");
    assert!(crypto.decrypt("v1", row.access_key_id, &encrypted).is_err());
    let plaintext = crypto.decrypt("v2", row.access_key_id, &encrypted).unwrap();
    assert_eq!(prepared.into_output().secret_key, plaintext.expose_secret());
}
