# Fresh Installation and Backup Restore

The current baseline initializes an empty PostgreSQL database. It is not an
in-place upgrade of FileGate or an earlier Grove schema. SQLx rejects a different
migration history; there are no old-name views or table aliases.

## Schema Ownership

| Baseline | Responsibility |
|---|---|
| `0001_time.sql` | Transaction-stable and wall-clock DB functions |
| `0002_registry.sql` | Storages, Clients, Native keys, S3 credentials |
| `0003_objects.sql` | Files, locations, leases, logical S3 keys, uploads and usage |
| `0004_management.sql` | Accounts, API tokens, password login, sessions and history |

The baseline creates 23 application tables.
S3-only storage and Account session ownership are defined directly, without
creating and later removing FS, User/Agent or Root/Master structures.
Legacy operator tables are absent. Retired management routes return 410.
History records Account, System or Anonymous actors, without Agent-owner fields.

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
  -> account init -> four baseline checksums
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
| Baseline before legacy retirement | Identifier-only catalog comparison passed; the final 23-table baseline is checked by `schema_baseline.rs` |
| Rust | Workspace tests passed; one native release-artifact test ignored. Configuration alias rejection passed separately |
| Fresh installation | Account bootstrap, Native/S3 bytes, exact backup restore and pending upload completion passed |
| File and management contracts | S3 SDK, presigned transfers, commit-failure recovery, CLI and MCP passed |
| Real HTTPS console | Registration, metadata, usage, roles, password/session/token lifecycle and CSRF passed |
| Browser regression | 503 passed with Rust-generated Swagger and one worker; lint and production build passed |

An earlier two-worker browser run failed two capacity assertions. Their isolated
rerun and the full one-worker run passed without a capacity-code change; the
initial failure cause remains unconfirmed. These results do not verify a built
release image, vulnerability scan, published CLI assets or a production rollout.
