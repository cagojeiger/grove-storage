//! 파일·lease·part의 상태 전이를 조건부 트랜잭션으로 처리한다.
//! 물리 I/O는 호출자가 수행하며 중계 PUT/part 승격은 claim 잠금으로 보호한다.
//! create·access·commit·completion·sweep·multipart가 각 생애주기 경로를 담당한다.

mod access;
mod commit;
mod completion;
mod create;
mod multipart;
mod reclaim_cleanup;
mod relay_upload;
mod sweep;

pub use access::{
    ByteLease, FileAccess, FileStat, access, attach_write_secret, byte_lease, issue_read_lease,
    recorded_upload, stat,
};
pub use commit::{
    ObservedCommitCandidate, finalize_commit, finalize_multipart_commit, observed_commit_candidates,
};
pub use completion::{
    CompletionCandidate, CompletionStart, begin_completion, claim_cleanup,
    cleanup_candidates as completion_cleanup_candidates, completion_candidates,
    finalize_cleanup as finalize_completion_cleanup, finalize_completion, renew_completion_lease,
    reopen_completion,
};
pub(crate) use create::create_in_tx;
pub use create::{CreateOutcome, CreateSpec, CreatedFile, create};
pub use multipart::{
    PartClaim, RelayPartClaim, WriteLease, attach_upload_id, cancel_relay_part, claim_part,
    claim_relay_part, done_parts, extend_write_lease, finish_relay_part, has_done_parts,
    record_part_done, renew_relay_part_lease, write_lease,
};
pub use reclaim_cleanup::{finalize_reclaim_cleanup, reclaim_cleanup_candidates};
pub use relay_upload::{RelayUploadClaim, claim_relay_upload};
pub use sweep::{
    DeleteOutcome, SweepCandidate, active_multipart_lease_ids, expire_read_leases, expired_pending,
    finalize_purge, finalize_reclaim, mark_deleted, prune_history, prune_terminal_files,
    prune_terminal_leases, purgeable, reclaim_pending,
};
