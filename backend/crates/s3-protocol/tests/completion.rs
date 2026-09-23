use grove_s3_protocol::completion::{CompletionError as Error, MIN_PART_BYTES, reconcile};

fn requested(numbers: &[i32]) -> Vec<(i32, String)> {
    numbers.iter().map(|n| (*n, format!("etag{n}"))).collect()
}

fn ledger(first_size: i64) -> Vec<(i32, i64, String)> {
    vec![(2, 1, "etag2".into()), (1, first_size, "etag1".into())]
}

#[test]
fn minimum_boundary_and_requested_order_are_preserved() {
    let result = reconcile(&requested(&[1, 2]), &ledger(MIN_PART_BYTES));
    assert_eq!(
        result,
        Ok(vec![
            (1, MIN_PART_BYTES, "etag1".into()),
            (2, 1, "etag2".into())
        ])
    );
    assert_eq!(
        reconcile(&requested(&[1, 2]), &ledger(MIN_PART_BYTES - 1)),
        Err(Error::EntityTooSmall)
    );
}

#[test]
fn last_requested_part_is_exempt_even_when_later_parts_exist() {
    for size in [0, 1, MIN_PART_BYTES - 1] {
        assert!(reconcile(&requested(&[1]), &ledger(size)).is_ok());
    }
    assert!(
        reconcile(
            &requested(&[1, 2]),
            &[(1, MIN_PART_BYTES, "etag1".into()), (2, 0, "etag2".into())]
        )
        .is_ok()
    );
}

#[test]
fn order_errors_are_distinct_from_size_errors() {
    for parts in [requested(&[2, 1]), requested(&[1, 1])] {
        assert_eq!(reconcile(&parts, &ledger(1)), Err(Error::InvalidPartOrder));
    }
    assert_eq!(Error::InvalidPartOrder.code(), "InvalidPartOrder");
    assert_eq!(Error::EntityTooSmall.code(), "EntityTooSmall");
}

#[test]
fn missing_mismatched_and_invalid_parts_are_rejected() {
    for parts in [
        requested(&[3]),
        requested(&[0]),
        requested(&[10001]),
        vec![],
        vec![(1, "wrong".into())],
    ] {
        assert_eq!(
            reconcile(&parts, &ledger(MIN_PART_BYTES)),
            Err(Error::InvalidPart)
        );
    }
}

#[test]
fn nonconsecutive_parts_are_valid_in_the_basic_etag_contract() {
    assert!(
        reconcile(
            &requested(&[1, 10000]),
            &[
                (1, MIN_PART_BYTES, "etag1".into()),
                (10000, 1, "etag10000".into())
            ]
        )
        .is_ok()
    );
}

#[test]
fn quoted_case_insensitive_etag_comparison_is_preserved() {
    assert_eq!(
        reconcile(&[(1, "\"ETAG1\"".into())], &ledger(1)),
        Ok(vec![(1, 1, "etag1".into())])
    );
}
