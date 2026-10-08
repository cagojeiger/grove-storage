# Fresh Installation and Backup Restore

The current baseline initializes an empty PostgreSQL database. It is not an
in-place upgrade of FileGate or a pre-0.5 Grove schema. SQLx rejects a different
migration history; there are no old-name views or table aliases.

## Schema Ownership

| Baseline | Responsibility |
|---|---|
| `0001_time.sql` | Transaction-stable and wall-clock DB functions |
| `0002_registry.sql` | Storages, Clients, Native keys, S3 credentials |
| `0003_objects.sql` | Files, locations, leases, logical S3 keys, uploads and usage |
| `0004_management.sql` | Accounts, API tokens, password login, sessions and history |
| `0005_retention_integrity.sql` | Client/file ownership, issued write uniqueness, history identity, observation time and cleanup indexes |
| `0006_upload_ownership.sql` | One durable Native/S3 upload owner per file; protocol-specific shape checks |
| `0007_upload_recovery.sql` | Durable retry eligibility and ordered completion/cleanup candidates |
| `0008_file_recovery.sql` | Retry eligibility for generic observation, reclaim cleanup and purge |

The current schema creates 22 application tables.
S3-only storage and Account session ownership are defined directly, without
creating and later removing FS, User/Agent or Root/Master structures.
Legacy operator tables are absent. Retired management routes return 410.
History records Account, System or Anonymous actors, without Agent-owner fields.
The released 0.5.0 migrations remain unchanged. Migration 0005 also applies to
that baseline; invalid ownership or duplicate issued writes reject the upgrade
atomically, without silently dropping rows. Existing observation times remain
unknown. Retention and file recovery have separate execution locks.
Migration 0006 preserves pending upload keys, conditional flags, completion values
and timestamps in `uploads`. Native/S3 owners of the same file reject the migration
atomically. Leases, part measurements and vendor upload IDs stay unchanged.
Migration 0007 keeps those intents and timestamps intact. Existing rows start due;
failed attempts retain their owner and defer the next recovery attempt.
Migration 0008 preserves file states, locations and leases. Existing files start
due; new cleanup phases reset eligibility rather than inherit observation delay.

## Names and Interfaces

| Scope | Contract |
|---|---|
| Server / remote CLI | `grove-storage` / `gscli` |
| Server configuration / internal crates | `GROVE_*` / `grove-*`; no `FILEGATE_*` aliases |
| Native and S3 access | Existing HTTP paths, SigV4 and presigned upload/download contracts |
| Management API tokens | DB `management.api_tokens`; existing credential issuance HTTP paths |
| Security formats | Existing cookie/token formats and encryption derivation labels |

File-request authentication remains independent of management Accounts.
An old encrypted secret does not become compatible with the new DB by renaming
its table. Importing old rows, preserving credentials and moving external objects
are separate from fresh installation and are not implemented by this baseline.

## Rehearsal

```text
Empty PostgreSQL + owned MinIO
  -> account init -> eight migration checksums
  -> password login -> resource registration
  -> Native and standard S3 SDK/presigned transfers
  -> active objects + pending Native/S3 multipart uploads
  -> stop server -> protected pg_dump -> restore to a new database
  -> exact rows/checksums -> start the same binary
  -> login, old fixture keys/URLs, reads and pending upload completion
  -> remove owned fixture containers and temporary files
```

```sh
# Docker and a Python environment with boto3==1.43.99
cargo build --bin grove-storage --locked
python3 -B -m unittest discover -s scripts/tests -v
python3 -B -u scripts/e2e-installation.py
```

CI job: `Fresh installation and backup restore`. The fixture accepts no external
database or storage endpoint and owns only the services it creates. Existing
local DBs, buckets and applications are outside its scope.

SQLx tests cover repeated migration, old-checksum rejection, failed-step rollback
and retry, S3-only registration, and the current table set. Real HTTPS browser
cookies/CSRF are covered separately by the console contract tests.

## Restore Boundary

Backup verification covers the same schema, encryption root/key ID and binary.
The fixture stops writers before backup and restores while provider bytes remain
available. A PostgreSQL dump does not contain S3 objects or undo subsequent S3
writes/deletes. Already issued direct S3 URLs can remain valid after server stop.
This rehearsal is local test evidence, not release publication or deployment.

## Local Verification

2026-10-07 working tree, macOS Command Line Tools, PostgreSQL 17, owned MinIO,
boto3 1.43.99 and Node 24.19.0:

| Scope | Result |
|---|---|
| Baseline before legacy retirement | Identifier-only catalog comparison passed; that checkpoint had 23 application tables |
| Rust | Workspace tests passed; one native release-artifact test ignored. Configuration alias rejection passed separately |
| Fresh installation | Account bootstrap, Native/S3 bytes, exact backup restore and pending upload completion passed |
| File and management contracts | S3 SDK, presigned transfers, commit-failure recovery, CLI and MCP passed |
| Real HTTPS console | Registration, metadata, usage, roles, password/session/token lifecycle and CSRF passed |
| Browser regression | 503 passed with Rust-generated Swagger and one worker; lint and production build passed |

An earlier two-worker browser run failed two capacity assertions. Their isolated
rerun and the full one-worker run passed without a capacity-code change; the
initial failure cause remains unconfirmed. These results do not verify a built
release image, vulnerability scan, published CLI assets or a production rollout.

### Retention and Integrity Checkpoint

2026-10-08 working tree, disposable PostgreSQL 17 and MinIO, boto3 1.43.99:

| Scope | Result |
|---|---|
| Rust | Workspace tests, Clippy with warnings denied and format check passed; native release-artifact updater test remains ignored |
| DB | Fresh install, 0.5.0 upgrade, atomic rejection of invalid ownership/duplicate writes, injected-time retention and audit rollback passed |
| Integration | `e2e-installation.py`, `e2e-s3.py` and `e2e-cli.py` passed: Native/S3 bytes, multipart, provider outage recovery, exact backup restore and pending upload resume |
| UI | 24 usage history/chart browser tests passed; desktop/mobile light/dark screenshots, lint and production build passed |
| Scripts | 24 Python script unit tests passed |

The macOS Rust run used Command Line Tools and `SSL_CERT_FILE=/etc/ssl/cert.pem`.
Native-root discovery initially failed without the explicit CA file; TLS
verification remained enabled. This retention-only checkpoint preceded migration
0006: it had 23 application tables and separate Native/S3 upload ownership.

### Upload Ownership Checkpoint

2026-10-08 working tree, disposable PostgreSQL 17 and MinIO, boto3 1.43.99:

| Scope | Result |
|---|---|
| Schema | 22 application tables and six migration checksums; Native/S3 recovery ownership shares `uploads` |
| DB | Six new ownership tests passed: protocol constraints, file-lock recheck, adapter isolation, upgrade preservation and atomic rejection of dual owners |
| Rust | Workspace tests, Clippy with warnings denied and format check passed; native release-artifact updater test remains ignored |
| Integration | `e2e-installation.py`, `e2e-s3.py`, `e2e-cli.py` and both `e2e-s3-recovery.py --restart` / `--db-failure` passed |
| Scripts | 24 Python script unit tests passed |

Integration covered standard S3 SDK/presigned transfers, Native/S3 pending upload
completion after backup restore, provider completion response loss, process restart
and DB commit failure. Recovery ownership survived each failure and activation
occurred once. Frontend behavior was unchanged and was not retested at this stage.
These are local working-tree results, not NoteGate, release-image or production
rollout verification.

### Recovery Hardening Checkpoint

2026-10-08 working tree, disposable PostgreSQL 17 and MinIO, boto3 1.43.99:

| Scope | Result |
|---|---|
| Schema | 22 application tables, seven migration checksums; 0007 preserves all four completion/cleanup owner shapes |
| DB | Seven retry tests passed: four-path candidate rotation, exact injected-time cutoff, concurrent claim, lock-wait rechecks and upgrade preservation |
| API timing | Three paused-time tests passed for the 10-second I/O deadline and success/error propagation |
| Rust | Workspace tests, warnings-denied Clippy and format check passed; the native release-artifact updater test remains ignored |
| Failure integration | `e2e-recovery-hardening.py` passed: actual COMMIT ACK loss, SIGKILL during provider reply wait and 40 failed candidates alongside healthy progress |
| Existing integration | `e2e-s3-recovery.py --restart` and `--db-failure` passed; conditional writes, old-object purge and usage settlement remain intact |
| Installation | `e2e-installation.py` passed with all seven checksums, exact backup restore, existing keys/URLs and pending Native/S3 completion |
| Scripts | 27 Python fixture/contract unit tests passed |

The 30-second delay is persisted before I/O. It is an eligibility time, not an
upper bound on recovery latency. At this checkpoint generic observation/reclaim/purge
rotation and the whole worker-pass budget remained separate work. Frontend, NoteGate, AWS/R2,
release images and production endpoints were not retested in this checkpoint.

### File Recovery and Pass Budget Checkpoint

2026-10-08 working tree, disposable PostgreSQL 17, HTTP provider fixtures and MinIO:

| Scope | Result |
|---|---|
| Schema | 22 application tables, eight migrations; 0008 preserves file states, locations and leases |
| DB | Seven generic retry tests passed: batch fairness, exact injected-time eligibility, concurrent claims, lock-wait rechecks, phase reset, stale purge rejection and upgrade preservation |
| Worker | Ten tests passed, including four paused-time scheduling tests, three I/O deadline tests and three real PostgreSQL/HTTP worker tests |
| Fairness | Each generic job processed a healthy candidate after 40 provider denials; all 120 failed intents recovered after repair at the injected retry deadline |
| Cancellation | Shutdown during physical cleanup retained location, lease and retry time; the advisory lock became available to another pass |
| Rust | Workspace tests passed with one native release-artifact updater test ignored; targeted tests, warnings-denied Clippy, format check and server/CLI build passed after final cleanup |
| Integration | `e2e-installation.py`, `e2e-recovery-hardening.py`, `e2e-s3-recovery.py --restart` and `--db-failure` passed with the new binary |
| Scripts | 27 Python fixture/contract tests passed |

The object pass uses a 20-second Tokio budget, including local spool, pool and
lock acquisition. Interrupted passes resume with the next job kind. Job order is
process-local; retry times survive restart in PostgreSQL. Cancellation does not
prove that a remote DeleteObject or an already-sent COMMIT was rolled back.
Cleanup is idempotent and only releases locations after physical success.
Frontend, NoteGate, AWS/R2, release images and production endpoints were not
retested. These are local results, not a merge, publication or deployment.
