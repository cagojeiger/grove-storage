#![allow(clippy::unwrap_used)]
mod support;

use grove_management_command::*;
use serde_json::json;

#[test]
fn all_result_shapes_round_trip_without_changing_the_public_payload() {
    for &name in CommandName::ALL {
        let fixture = support::output(name);
        let decoded = name.decode_output(fixture.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), fixture, "{name:?}");
        assert_eq!(
            name.decode_output(json!(null)).unwrap_err(),
            ErrorCode::InvalidResponse
        );
    }
}

#[test]
fn storage_output_excludes_secret_and_encryption_fields() {
    let mut fixture = support::storage();
    for key in [
        "secret_key",
        "secret_key_ciphertext",
        "secret_key_nonce",
        "enc_key_id",
    ] {
        fixture
            .as_object_mut()
            .unwrap()
            .insert(key.into(), json!(support::SECRET));
    }
    let output = CommandName::StorageShow.decode_output(fixture).unwrap();
    let result = serde_json::to_value(output).unwrap();
    assert_eq!(result, support::storage());
    assert!(!result.to_string().contains(support::SECRET));
}

#[test]
fn secret_delivery_is_separate_from_debug_and_input_errors() {
    let request = decode(
        1,
        "storage.create",
        support::input(CommandName::StorageCreate),
    )
    .unwrap();
    assert!(!format!("{request:?}").contains(support::SECRET));
    let response = CommandName::CredentialCreate
        .decode_output(support::output(CommandName::CredentialCreate))
        .unwrap();
    assert!(!format!("{response:?}").contains(support::SECRET));
    assert_eq!(
        serde_json::to_value(response).unwrap().get("secret_key"),
        Some(&json!(support::SECRET))
    );
    let error = decode(1, "usage.history", json!({"days":support::SECRET})).unwrap_err();
    assert!(!format!("{error:?}").contains(support::SECRET));
    assert!(
        !serde_json::to_string(&error)
            .unwrap()
            .contains(support::SECRET)
    );
}

#[test]
fn status_never_claims_a_physical_probe_and_usage_keeps_negative_remaining() {
    let mut fixture = support::output(CommandName::Status);
    *fixture.get_mut("storage_access").unwrap() = json!("ok");
    assert_eq!(
        CommandName::Status.decode_output(fixture).unwrap_err(),
        ErrorCode::InvalidResponse
    );
    let output = CommandName::UsageStorages
        .decode_output(support::output(CommandName::UsageStorages))
        .unwrap();
    assert_eq!(
        serde_json::to_value(output)
            .unwrap()
            .pointer("/0/remaining_bytes"),
        Some(&json!(-1))
    );
}
