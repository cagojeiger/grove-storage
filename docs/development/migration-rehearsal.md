# Offline Migration Rehearsal

## Boundary

| Item | Contract |
|---|---|
| Source | FileGate `v0.4.1`, commit `af685807e5561885616e00918f0207e25188fb3a`, migrations `0001..0006` |
| Target | Current Grove binary and its embedded migrations |
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
  -> start Grove -> old keys/URLs read old bytes + password Admin login
  -> stop Grove -> assert external S3 objects and multipart parts unchanged
       |
       v
Old binary against upgraded DB -> migration rejection
Backup -> fresh rollback DB -> exact legacy rows and migration checksums
  -> start FileGate -> read old files -> resume pending uploads -> verify bytes
  -> remove fixture services, temporary dump and logs
```

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
Password login is an API assertion here. Actual browser HTTPS/cookie behavior is
covered by the separate `Console real HTTPS API contract` job step.

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
