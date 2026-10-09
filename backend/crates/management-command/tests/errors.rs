#![allow(clippy::unwrap_used)]
use grove_management_command::*;
use serde_json::json;

#[test]
fn unsupported_protocol_is_rejected_before_name_or_input_validation() {
    for version in [0, 2, u16::MAX] {
        assert_eq!(
            decode(version, "user.create", json!(null)).unwrap_err(),
            CommandError::rejected(ErrorCode::ProtocolIncompatible)
        );
    }
    assert_eq!(
        decode(1, "user.create", json!(null)).unwrap_err(),
        CommandError::rejected(ErrorCode::UnknownCommand)
    );
}

#[test]
fn mutation_outcome_is_preserved_independently_of_the_error_code() {
    for outcome in [Outcome::NotApplied, Outcome::Applied, Outcome::Unknown] {
        let error = CommandError {
            code: ErrorCode::Unavailable,
            outcome,
        };
        let encoded = serde_json::to_value(error).unwrap();
        assert_eq!(
            serde_json::from_value::<CommandError>(encoded).unwrap(),
            error
        );
    }
}

#[test]
fn errors_have_closed_fields_and_stable_codes() {
    assert_eq!(
        serde_json::to_value(CommandError::rejected(ErrorCode::RateLimited)).unwrap(),
        json!({"code":"rate_limited", "outcome":"not_applied"})
    );
    assert_eq!(
        serde_json::to_value(CommandError::rejected(ErrorCode::Forbidden)).unwrap(),
        json!({"code":"forbidden", "outcome":"not_applied"})
    );
    assert!(
        serde_json::from_value::<CommandError>(
            json!({"code":"forbidden", "outcome":"not_applied", "secret":"raw"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<CommandError>(
            json!({"code":"provider-secret-error", "outcome":"not_applied"})
        )
        .is_err()
    );
}
