use grove_object_policy::completion::{CompletionAction, ObjectObservation, completion_action};

#[test]
fn matching_objects_finalize_for_both_upload_modes() {
    for multipart in [false, true] {
        for etag in [None, Some("abc-2"), Some("ABC-2")] {
            let observation = Some(ObjectObservation {
                size: 12,
                etag: etag.map(str::to_owned),
            });
            assert_eq!(
                completion_action::<()>(12, "abc-2", multipart, Ok(observation)),
                Ok(CompletionAction::Finalize)
            );
        }
    }
}

#[test]
fn missing_multipart_reopens_but_missing_single_upload_cleans_up() {
    assert_eq!(
        completion_action::<()>(12, "abc", true, Ok(None)),
        Ok(CompletionAction::Reopen)
    );
    assert_eq!(
        completion_action::<()>(12, "abc", false, Ok(None)),
        Ok(CompletionAction::Cleanup)
    );
}

#[test]
fn size_or_etag_mismatch_requires_cleanup() {
    for multipart in [false, true] {
        for (size, etag) in [
            (11, None),
            (13, Some("abc")),
            (12, Some("wrong")),
            (12, Some("")),
        ] {
            assert_eq!(
                completion_action::<()>(
                    12,
                    "abc",
                    multipart,
                    Ok(Some(ObjectObservation {
                        size,
                        etag: etag.map(str::to_owned),
                    }))
                ),
                Ok(CompletionAction::Cleanup)
            );
        }
    }
}

#[test]
fn empty_single_object_is_not_a_missing_object() {
    assert_eq!(
        completion_action::<()>(
            0,
            "empty",
            false,
            Ok(Some(ObjectObservation {
                size: 0,
                etag: None
            }))
        ),
        Ok(CompletionAction::Finalize)
    );
}
