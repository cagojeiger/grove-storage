#![allow(clippy::unwrap_used, clippy::panic)]
mod support;

use grove_management_command::*;
use serde_json::json;

#[test]
fn typed_commands_serialize_to_the_same_input_contract() {
    for &name in CommandName::ALL {
        let command = decode(1, name.as_str(), support::input(name)).unwrap();
        let wire = serde_json::to_value(&command).unwrap();
        assert!(wire.is_object());
        assert_eq!(decode(1, name.as_str(), wire).unwrap().name(), name);
        assert!(!format!("{command:?}").contains(support::SECRET));
    }
}

#[test]
fn unknown_fields_null_and_nonobject_inputs_are_rejected_for_every_command() {
    for &name in CommandName::ALL {
        let mut extra = support::input(name);
        extra
            .as_object_mut()
            .unwrap()
            .insert("role".into(), json!("admin"));
        for input in [extra, json!(null), json!([]), json!("raw-token")] {
            assert_eq!(
                decode(1, name.as_str(), input).unwrap_err(),
                CommandError::rejected(ErrorCode::InvalidInput),
                "{name:?}"
            );
        }
    }
}

#[test]
fn required_fields_and_nested_storage_extras_are_rejected() {
    for &name in CommandName::ALL {
        let original = support::input(name);
        for key in original.as_object().unwrap().keys() {
            if key == "days" {
                continue;
            }
            let mut missing = original.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(decode(1, name.as_str(), missing).is_err(), "{name:?} {key}");
        }
    }
    for extra in ["id", "role", "enc_key_id", "secret_key_ciphertext"] {
        let mut input = support::input(CommandName::StorageCreate);
        input
            .get_mut("spec")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(extra.into(), json!("hidden"));
        assert!(decode(1, "storage.create", input).is_err());
    }
    for spec in [
        json!(null),
        json!([]),
        json!([
            "s3", false, null, null, null, null, null, false, null, null, 0
        ]),
    ] {
        assert!(decode(1, "storage.create", json!({"id":"archive", "spec":spec})).is_err());
    }
}

#[test]
fn history_defaults_and_bounds_match_the_cli() {
    let Command::UsageHistory(input) = decode(1, "usage.history", json!({})).unwrap() else {
        panic!("history input")
    };
    assert_eq!(input.days, 90);
    for days in [1, 3650] {
        assert!(decode(1, "usage.history", json!({"days":days})).is_ok());
    }
    for days in [
        json!(0),
        json!(3651),
        json!(-1),
        json!(65536),
        json!(1.5),
        json!("90"),
        json!(null),
    ] {
        assert!(decode(1, "usage.history", json!({"days":days})).is_err());
    }
}

#[test]
fn ids_are_lookup_values_not_local_paths_or_dot_segments() {
    for id in ["", ".", "..", "app\n", "\u{0000}"] {
        assert!(decode(1, "storage.show", json!({"id":id})).is_err());
        assert!(decode(1, "client.create", json!({"id":"app", "storage_id":id})).is_err());
    }
    // Preserve the existing CLI lookup/encoding behavior, not creation slugs.
    assert!(decode(1, "client.show", json!({"id":"a/b?#%"})).is_ok());
}

#[test]
fn client_key_is_a_hash_not_a_raw_secret() {
    for name in ["client-key.register", "client-key.delete"] {
        assert!(
            decode(
                1,
                name,
                json!({"client_id":"app", "key_hash":support::hash()})
            )
            .is_ok()
        );
        for hash in [
            support::SECRET.into(),
            format!("sha256:{}", "A".repeat(64)),
            format!("sha256:{}", "a".repeat(63)),
        ] {
            assert!(decode(1, name, json!({"client_id":"app", "key_hash":hash})).is_err());
        }
        assert!(
            decode(
                1,
                name,
                json!({"client_id":"app", "key_file":"/private/key"})
            )
            .is_err()
        );
    }
}

#[test]
fn credential_ids_preserve_ascii_length_rules() {
    for length in [8, 64] {
        assert!(
            decode(
                1,
                "credential.delete",
                json!({"client_id":"app", "access_key_id":"a".repeat(length)})
            )
            .is_ok()
        );
    }
    for id in [
        "a".repeat(7),
        "a".repeat(65),
        "AAAAAAAA".into(),
        "abcdefg/".into(),
    ] {
        assert!(
            decode(
                1,
                "credential.delete",
                json!({"client_id":"app", "access_key_id":id})
            )
            .is_err()
        );
    }
}

#[test]
fn storage_defaults_and_capacity_shape_are_shared_without_running_a_probe() {
    let input =
        json!({"id":"local", "spec":{"kind":"fs", "root_path":"/not-opened", "capacity_bytes":0}});
    assert!(decode(1, "storage.create", input).is_ok());
    let Command::StorageCreate(input) = decode(
        1,
        "storage.create",
        json!({"id":"s3", "spec":{"capacity_bytes":1}}),
    )
    .unwrap() else {
        panic!("storage input")
    };
    assert_eq!(input.spec.kind, model::StorageKind::S3);
    assert!(!input.spec.force_relay);
    assert!(!input.spec.force_path_style);
    // Provider-specific requirements are still enforced by the service.
    for capacity in [json!(-1), json!(1.5), json!("1"), json!(null)] {
        assert!(
            decode(
                1,
                "storage.create",
                json!({"id":"s3", "spec":{"capacity_bytes":capacity}})
            )
            .is_err()
        );
    }
}
