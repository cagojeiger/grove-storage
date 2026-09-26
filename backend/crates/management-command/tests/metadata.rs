#![allow(clippy::unwrap_used)]
use grove_management_command::{decode, metadata::valid_metadata};
use serde_json::json;

#[test]
fn metadata_is_a_bounded_string_object_not_arbitrary_json() {
    for value in [
        json!({}),
        json!({"description":"notes", "owner":"team"}),
        json!({"label":"한글"}),
    ] {
        assert!(valid_metadata(&value));
        for resource in ["storage", "client"] {
            decode(
                1,
                &format!("{resource}.metadata.replace"),
                json!({"id":"app","metadata":value}),
            )
            .unwrap();
        }
    }
    for value in [
        json!(null),
        json!([]),
        json!("text"),
        json!({"x":null}),
        json!({"x":1}),
        json!({"x":true}),
        json!({"x":{}}),
        json!({"x":[]}),
        json!({"x":"\u{0}"}),
        json!({"\u{0}":"x"}),
    ] {
        assert!(!valid_metadata(&value));
        assert!(
            decode(
                1,
                "client.metadata.replace",
                json!({"id":"app","metadata":value})
            )
            .is_err()
        );
    }
}

#[test]
fn size_limit_matches_normalized_utf8_json() {
    // PostgreSQL renders {"x": "..."}: nine bytes plus the value.
    assert!(valid_metadata(&json!({"x":"a".repeat(8183)})));
    assert!(!valid_metadata(&json!({"x":"a".repeat(8184)})));
    assert!(valid_metadata(&json!({"x":"한".repeat(2727)})));
    assert!(!valid_metadata(&json!({"x":"한".repeat(2728)})));
    assert!(!valid_metadata(&json!({"x":"\n".repeat(4092)})));
}
