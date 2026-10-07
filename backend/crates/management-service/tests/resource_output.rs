#![allow(clippy::unwrap_used)]
use grove_db::usage;
use grove_management_service::{
    Error,
    resources::{client_usage_output, snapshot_output, storage_usage_output},
};
use serde_json::json;

fn row() -> usage::StorageUsage {
    usage::StorageUsage {
        storage_id: "home".into(),
        kind: "s3".into(),
        capacity_bytes: 100,
        reserved_bytes: 10,
        active_bytes: 120,
        purge_pending_bytes: 20,
        reserved_files: 1,
        active_files: 2,
        purge_pending_files: 3,
    }
}

#[test]
fn usage_keeps_negative_remaining_and_every_accounting_field() {
    let result = storage_usage_output(row()).unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "storage_id":"home","kind":"s3","capacity_bytes":100,
            "reserved_bytes":10,"active_bytes":120,"purge_pending_bytes":20,
            "remaining_bytes":-50,"reserved_files":1,"active_files":2,"purge_pending_files":3
        })
    );
}

#[test]
fn usage_accepts_the_exact_integer_boundary_and_rejects_overflow() {
    let mut value = row();
    value.capacity_bytes = 0;
    value.reserved_bytes = i64::MAX;
    value.active_bytes = 1;
    value.purge_pending_bytes = 0;
    assert_eq!(
        storage_usage_output(value).unwrap().remaining_bytes,
        i64::MIN
    );
    for (active, purge) in [(2, 0), (1, 1)] {
        let mut value = row();
        value.capacity_bytes = 0;
        value.reserved_bytes = i64::MAX;
        value.active_bytes = active;
        value.purge_pending_bytes = purge;
        assert!(matches!(
            storage_usage_output(value),
            Err(Error::Unavailable)
        ));
    }
}

#[test]
fn client_and_history_projections_preserve_identity_counts_and_date_format() {
    let client = client_usage_output(usage::ClientUsage {
        client_id: "app".into(),
        storage_id: "home".into(),
        active_bytes: 120,
        active_files: 2,
    });
    assert_eq!(
        serde_json::to_value(client).unwrap(),
        json!({
            "client_id":"app","storage_id":"home","active_bytes":120,"active_files":2
        })
    );
    let snapshot = snapshot_output(usage::SnapshotRow {
        day: chrono::NaiveDate::from_ymd_opt(2026, 9, 27).unwrap(),
        client_id: "app".into(),
        storage_id: "home".into(),
        active_bytes: 120,
        active_files: 2,
    });
    assert_eq!(
        serde_json::to_value(snapshot).unwrap(),
        json!({
            "day":"2026-09-27","client_id":"app","storage_id":"home","active_bytes":120,"active_files":2
        })
    );
}
