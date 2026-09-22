use grove_object_policy::completion::{CompletionAction, ObjectObservation, completion_action};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservationFailure {
    Timeout,
    PermissionDenied,
    Unavailable,
}

#[test]
fn failed_observation_never_becomes_a_transition() {
    for multipart in [false, true] {
        for error in [
            ObservationFailure::Timeout,
            ObservationFailure::PermissionDenied,
            ObservationFailure::Unavailable,
        ] {
            let result = completion_action(12, "abc", multipart, Err(error));
            assert_eq!(result, Err(error));
        }
    }
}

#[test]
fn retry_after_observation_failure_uses_fresh_evidence() {
    for multipart in [false, true] {
        let observations = [
            Err(ObservationFailure::Timeout),
            Err(ObservationFailure::Unavailable),
            Ok(Some(ObjectObservation {
                size: 12,
                etag: Some("ABC".to_owned()),
            })),
        ];
        let actions: Vec<_> = observations
            .into_iter()
            .filter_map(|observation| completion_action(12, "abc", multipart, observation).ok())
            .collect();
        assert_eq!(actions, [CompletionAction::Finalize]);
    }
}

#[test]
fn missing_object_after_a_failed_observation_keeps_mode_specific_behavior() {
    for (multipart, expected) in [
        (true, CompletionAction::Reopen),
        (false, CompletionAction::Cleanup),
    ] {
        assert_eq!(
            completion_action(
                12,
                "abc",
                multipart,
                Err(ObservationFailure::PermissionDenied)
            ),
            Err(ObservationFailure::PermissionDenied)
        );
        assert_eq!(
            completion_action::<ObservationFailure>(12, "abc", multipart, Ok(None)),
            Ok(expected)
        );
    }
}
