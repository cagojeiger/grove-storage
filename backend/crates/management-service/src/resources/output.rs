//! Public resource projections shared by legacy REST and command transports.
use crate::Error;
use filegate_db::{registry::StorageRow, usage};
use grove_management_command::model::{self, StorageKind};

fn kind(value: &str) -> Result<StorageKind, Error> {
    match value {
        "s3" => Ok(StorageKind::S3),
        "fs" => Ok(StorageKind::Fs),
        _ => Err(Error::Unavailable),
    }
}

pub fn storage_output(row: StorageRow) -> Result<model::Storage, Error> {
    Ok(model::Storage {
        id: row.id,
        kind: kind(&row.kind)?,
        force_relay: row.force_relay,
        root_path: None,
        endpoint: row.endpoint,
        public_endpoint: row.public_endpoint,
        region: row.region,
        bucket: row.bucket,
        force_path_style: row.force_path_style,
        access_key: row.access_key,
        capacity_bytes: row.capacity_bytes,
    })
}

pub fn storage_usage_output(row: usage::StorageUsage) -> Result<model::StorageUsage, Error> {
    Ok(model::StorageUsage {
        storage_id: row.storage_id,
        kind: kind(&row.kind)?,
        capacity_bytes: row.capacity_bytes,
        reserved_bytes: row.reserved_bytes,
        active_bytes: row.active_bytes,
        purge_pending_bytes: row.purge_pending_bytes,
        remaining_bytes: row
            .capacity_bytes
            .checked_sub(row.reserved_bytes)
            .and_then(|n| n.checked_sub(row.active_bytes))
            .and_then(|n| n.checked_sub(row.purge_pending_bytes))
            .ok_or(Error::Unavailable)?,
        reserved_files: row.reserved_files,
        active_files: row.active_files,
        purge_pending_files: row.purge_pending_files,
    })
}

pub fn client_usage_output(row: usage::ClientUsage) -> model::ClientUsage {
    model::ClientUsage {
        client_id: row.client_id,
        storage_id: row.storage_id,
        active_files: row.active_files,
        active_bytes: row.active_bytes,
    }
}

pub fn snapshot_output(row: usage::SnapshotRow) -> model::Snapshot {
    model::Snapshot {
        day: row.day.to_string(),
        storage_id: row.storage_id,
        client_id: row.client_id,
        active_bytes: row.active_bytes,
        active_files: row.active_files,
    }
}
