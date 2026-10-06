# Offline Migration Rehearsal

## Boundary

| Item | Contract |
|---|---|
| Source | FileGate `v0.4.1`, commit `af685807e5561885616e00918f0207e25188fb3a`, migrations `0001..0006` |
| Target | Current Grove binary; all embedded migration versions/checksums must match current SQL sources |
| Services | Owned PostgreSQL 17 and MinIO containers, loopback only |
| Data | Fixtures created through the released FileGate HTTP APIs |
| Backup | Private temporary custom-format `pg_dump`; `pg_restore` into a new DB |
| Secrets | Same fixture encryption root/key ID before and after migration |
| Traffic | Each server stops before the next phase; no concurrent writers |
| Rollback | Restore the pre-upgrade DB, then start FileGate; image-only rollback is rejected |
| Scope | Rehearsal, not a production migration command; no database URL or storage endpoint input |

The local FileGate working tree may contain unreleased SQL. The pinned release,
not that working tree, defines the baseline. Release `v0.4.1` has six migrations;
the older `management_upgrade.rs` test starts from the later seven-migration schema.

## Flow

```text
FileGate v0.4.1
  -> register S3 + Client + Native/S3 keys
  -> write active files + leave pending Native/S3 multipart uploads
  -> stop server -> check schema checksums and recovery owners -> pg_dump
       |
       v
Grove local account init -> migrate + first password Admin
  -> compare legacy resource rows, encrypted keys, locations, leases, upload IDs
  -> check all target migration checksums and Account/Session schema
  -> start Grove -> old keys/URLs read old bytes + password Admin login
  -> stop Grove -> assert external S3 objects and multipart parts unchanged
       |
       v
Old binary against upgraded DB -> migration rejection
  |
  +-- Run 1: backup -> fresh rollback DB -> exact legacy rows/checksums
  |     -> FileGate -> read old files -> resume pending uploads -> verify bytes
  |
  +-- Run 2: independently repeat seed/backup/upgrade on new PG + MinIO
        -> Grove -> finish old pending Native/S3 uploads -> verify bytes

Each run removes its fixture services, temporary dump and server logs.
```

The forward-recovery run uses separate provider objects and databases. Completing
uploads changes S3, so it must not reuse the frozen rollback fixture. Old Native
upload URLs, service keys, S3 credentials, multipart upload IDs and part ETags are
used without reissuance. Previously issued S3 download URLs remain readable.

The fixture explicitly enables `FILEGATE_LEGACY_ADMIN_ENABLED` with a test-only
operator token to compare old registration API reads. This is not the production
default; password Account login is verified independently. Public S3 requests do
not acquire a management Account dependency.

## Run

Build a clean checkout of the pinned FileGate revision in a separate directory.
Both builds use their checked-in toolchain and lockfile; leave `CARGO_TARGET_DIR`
unset when building FileGate so its binary is at `target/debug/filegate`.

```sh
# In the pinned FileGate checkout
cargo build --bin filegate --locked

# In Grove; Python environment includes boto3==1.43.99 and Docker is running
cargo build --bin filegate --locked
python3 -B -m unittest discover -s scripts/tests -v
python3 -B -u scripts/e2e-migration.py --filegate-dir /path/to/pinned-filegate
```

CI job: `Offline FileGate migration rehearsal`. The Python entry point owns all
servers and containers it stops; the legacy checkout is read-only during the run.
The entry point executes both runs and reports the legacy revision and both binary
SHA-256 hashes. These identify local build artifacts, not a published image.
Password login is an API assertion here. Actual browser HTTPS/cookie behavior is
covered by the separate `Console real HTTPS API contract` job step.

## Local Evidence

On 2026-10-05 both runs passed using freshly built locked FileGate and Grove
binaries, isolated PostgreSQL 17 and pinned MinIO. All 24 target migrations
matched their source checksums, obsolete Master/Root authentication tables and
columns were absent, and Session ownership was a non-null `account_id`.
The 27 Python script tests also passed, including rejection of a stale target,
failed/checksum-mismatched migrations and obsolete authentication schema.

This evidence is from a dirty Grove development checkout. It must be repeated
from the eventual clean release candidate; it is not immutable release or live
cluster evidence.

## Production Gate

| Before a real cutover | Evidence still required |
|---|---|
| Inventory | Actual deployed binary/image, migration checksums, S3-only rows, encryption key IDs |
| Write freeze | Stop applications and reconcilers; account for outstanding presigned URLs that can still write directly to S3 |
| Recovery | Resolve `completing`/`cleaning` operations; explicitly inventory remaining open uploads and leases |
| Backup | Restore a protected production DB backup in isolation; preserve encryption secrets separately |
| External bytes | Quiescent objects/parts or a provider-specific recovery strategy; PostgreSQL restore alone does not undo S3 writes/deletes |
| Rollback window | Keep writes frozen until validation; after resuming writes, assess DB and S3 divergence before restoring |
| Console | Build/serve static UI and validate real Ingress/TLS routing separately |

This rehearsal establishes the stopped-writer fixture contract. It does not
authorize production changes or establish recovery for mutations made after backup.
