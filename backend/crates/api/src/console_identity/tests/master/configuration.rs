use super::*;

#[test]
fn configuration_is_optional_paired_validated_and_errors_exclude_secrets() {
    use crate::console_identity::master_config::load;
    assert!(load(&|_| None).unwrap().is_none());
    let good = format!("gsmt_{}", "a".repeat(64));
    for (token, generation) in [
        (Some(good.as_str()), Some("1")),
        (Some("private-invalid-secret"), Some("1")),
        (Some(good.as_str()), None),
        (None, Some("1")),
        (Some(good.as_str()), Some("0")),
        (Some(good.as_str()), Some("9223372036854775808")),
    ] {
        let result = load(&|key| match key {
            "FILEGATE_MASTER_TOKEN" => token.map(str::to_owned),
            "FILEGATE_MASTER_GENERATION" => generation.map(str::to_owned),
            _ => None,
        });
        if token == Some(good.as_str()) && generation == Some("1") {
            assert!(result.is_ok());
        } else {
            assert!(
                !result
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("private-invalid-secret")
            );
        }
    }
    assert_ne!(secrets::master_hash(&good), secrets::token_hash(&good));
    assert_ne!(
        secrets::master_session_hash(&good),
        secrets::session_hash(&good)
    );
}
