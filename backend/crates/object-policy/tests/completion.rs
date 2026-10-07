use grove_object_policy::completion::{CompletionAction, ObjectObservation, completion_action};

#[test]
fn matching_objects_finalize_for_both_upload_modes() {
    for multipart in [false, true] {
        for etag in ["abc-2", "ABC-2"] {
            let observation = Some(ObjectObservation {
                size: 12,
                etag: etag.to_owned(),
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
        for (size, etag) in [(11, "abc"), (13, "abc"), (12, "wrong"), (12, "")] {
            assert_eq!(
                completion_action::<()>(
                    12,
                    "abc",
                    multipart,
                    Ok(Some(ObjectObservation {
                        size,
                        etag: etag.to_owned(),
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
                etag: "empty".to_owned()
            }))
        ),
        Ok(CompletionAction::Finalize)
    );
}
