//! S3 provider I/O and disposable local transfer spools.
//! Backend operations consume resolved settings; registry writes, authorization,
//! and object metadata transitions belong to their callers.

pub mod backend;
mod s3;
pub mod temp_spool;

pub use s3::{
    Address, S3ClientCache, S3Storage, S3StorageSpec, abort_multipart as s3_abort_multipart,
    abort_multipart_by_key as s3_abort_multipart_by_key,
    complete_multipart as s3_complete_multipart, connect as s3_connect,
    create_multipart as s3_create_multipart, delete_object as s3_delete_object,
    head_object as s3_head_object, list_parts as s3_list_parts, open_read as s3_open_read,
    open_read_range as s3_open_read_range, presign_get as s3_presign_get,
    presign_put as s3_presign_put, presign_upload_part as s3_presign_upload_part,
    put_object_from_path as s3_put_object_from_path, rfc5987_encode,
    upload_part_from_path as s3_upload_part_from_path,
};
