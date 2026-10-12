# FileGate to Grove Storage

The offline importer copies FileGate 0.4.1 resource records into the current Grove
baseline. Physical S3 objects stay in their existing buckets. File UUIDs, Client
IDs, logical keys, physical object keys and Client credentials remain unchanged.

## Data Mapping

| FileGate | Grove | Conversion |
|---|---|---|
| `storages` | `storages` | S3 fields preserved; secrets reencrypted; metadata starts empty |
| `clients` | `clients` | Storage assignment preserved; metadata starts empty |
| `client_keys` | `client_native_keys` | Existing key hashes preserved |
| `s3_credentials` | `client_s3_credentials` | Access/secret values preserved; ciphertext reencrypted |
| `files`, `locations`, `leases`, `lease_parts` | Same names | Existing IDs, states, measurements and timestamps preserved |
| `s3_keys` | `s3_object_keys` | Client/key/file mapping preserved |
| `lease_history` | `lease_history` | Events, including duplicates, preserved; new row IDs generated |
| `usage_snapshot` | `usage_snapshots` | Daily values preserved; legacy `observed_at` remains NULL |
| `s3_uploads`, `native_multipart_completions` | `uploads` | Drained source has no upload owners |

Management Accounts, password credentials, sessions and management history stay
in the destination database. File recovery eligibility starts at import time.
Normal Grove retention resumes after startup.

## Importer

```sh
cargo build -p grove-api --example filegate_migration --locked
target/debug/examples/filegate_migration export /private/path/filegate.json
target/debug/examples/filegate_migration import /private/path/filegate.json
target/debug/examples/filegate_migration verify /private/path/filegate.json
```

This maintenance executable runs from the source checkout. It is separate from
the published server image and `gscli`.

| Command | Environment | Behavior |
|---|---|---|
| `export` | `FILEGATE_DATABASE_URL` | Repeatable-read snapshot; six exact migration checksums; private new file |
| `import` | `GROVE_DATABASE_URL`, both encryption roots and key IDs | Locked, empty resource tables; one transaction; compare before commit |
| `verify` | Same as `import` | Read-only row and decrypted credential comparison |

Encryption variables are `FILEGATE_ENC_ROOT_SECRET`, `FILEGATE_ENC_KEY_ID`,
`GROVE_ENC_ROOT_SECRET` and `GROVE_ENC_KEY_ID`. Key IDs default to `v1`.
Secrets enter the process through environment variables. The snapshot contains
ciphertext and Native key hashes; its Unix permissions are `0600`. Existing files
are never overwritten. Runtime output reports table counts, without credential
values or PostgreSQL row details.

The importer accepts the exact FileGate 0.4.1 migration history and all current
Grove migration checksums. FS registrations, pending files, issued leases,
completion owners, cleanup locations, missing active locations, other destination
DB connections and populated destination resource tables stop the import.
Constraint or credential failures roll back the resource transaction. A lost
COMMIT response leaves the outcome unconfirmed; `verify` resolves that state
before another import attempt.

## Cutover

```text
NoteGate and other writers suspend file operations
  -> FileGate drains uploads and outstanding leases
  -> FileGate and Grove stop
  -> final DB backups + frozen source export
  -> import into Grove + exact verification
  -> Grove starts + internal S3 SDK/presigned verification
  -> object hostname routes to Grove
  -> public S3 SDK/presigned verification
  -> NoteGate resumes file operations + upload/download verification
```

Writer quiescence can be enforced by stopping consumers or by an operator who
controls every user and background writer. Keeping NoteGate running does not make
the file API available while the object servers are stopped.

With quick-deploy 0.1.19, GitOps maintenance sets each object application to
`enabled: false`. Its Deployment template defaults a zero `replicaCount` to one.
Pod absence and zero application DB connections confirm the frozen phase;
namespaces, ExternalSecrets and databases remain. A direct scale operation alone
can be undone by Argo self-heal. The stopped FileGate process also stops its
reconciler. Previously issued vendor URLs can outlive the application; the drain
includes their lease expiry. Only Grove is enabled after the import.

The current NoteGate endpoint is `https://filegate.project-jelly.io`, bucket
`notegate-prd`, region `us-east-1`, with path-style SigV4. Routing that hostname to
Grove keeps NoteGate's endpoint and credentials unchanged during the first cutover.
The management hostname remains `https://grove-storage.project-jelly.io`, protected
by Cloudflare Access. S3 traffic uses its own hostname and SigV4 authentication.
The public URL and allowed browser origins follow the existing object endpoint.

Post-import checks compare object count, size, ETag, logical key and physical
location; HEAD, GET, Range and presigned GET verify reads. A dedicated temporary
key verifies presigned PUT, HEAD, GET and DELETE. The probe operates through the
standard S3 interface. NoteGate's own upload/download flow completes the cutover.

The read-only comparison uses boto3 1.43.99 and the same Client credential for
both endpoints. `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` supply it privately.

```sh
python3 -B scripts/verify-filegate-migration.py \
  --snapshot /private/path/filegate.json \
  --source https://filegate.project-jelly.io \
  --target http://127.0.0.1:8080 \
  --bucket notegate-prd --report /private/path/read-report.json
```

This checks every snapshot key's HEAD size/ETag/content type and up to
4 KiB per object through presigned GET/Range. Source/target byte prefixes are
compared when `--source` is present, together with optional LastModified headers.
Post-cutover target checks omit that option. Exact timestamps are compared in SQL.
ListObjects stays outside the supported S3 contract; exact inventory comparison
belongs to the DB importer. The report contains counts and the
snapshot hash, without logical object names or signed URLs. It verifies readable
physical locations and the S3 interface; it is not a full-body checksum scan.

## Rollback

Before new Grove writes, rollback restores the object-host route to
FileGate and restarts its frozen deployment. Source DB and S3 bytes are unchanged
by the importer. New Grove writes end that simple rollback window: the two DBs
then describe different object lifecycles. Recovery after new writes includes
those changes and their physical objects, rather than only restoring a DB dump.

The backups cover both databases and their corresponding encryption-key
references. A PostgreSQL backup contains no provider bytes. Recovery retains the
provider buckets and immutable object keys throughout the cutover.

## Verification

```sh
DATABASE_URL=postgres://... \
  cargo test -p grove-api --example filegate_migration --locked
```

Tests use the exact six FileGate 0.4.1 SQL files under the example's `legacy`
fixture directory. They cover a new encryption root, duplicate history, unknown
legacy observation times, preserved management records, atomic rollback, repeated
import rejection, changed mappings, connection guards and unsupported source
states. CI runs this example explicitly with PostgreSQL 17.

The [fresh installation rehearsal](../development/fresh-installation.md) covers
the Grove baseline and backup restoration. The
[S3 contract](../spec/03-s3-surface.md) defines the consumer interface.
