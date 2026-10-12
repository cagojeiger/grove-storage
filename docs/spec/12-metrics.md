# Metrics

Metrics describe requests, byte transfer, provider calls, background jobs and
cached state. PostgreSQL remains the object/identity/audit authority. Metrics are
best-effort observations, not accounting or audit records.

## Exposure

`GROVE_METRICS_BIND` enables a separate listener, such as `0.0.0.0:9091`.
The default is disabled. Its only route is `GET /metrics`, using Prometheus text
exposition. The public S3/console listener does not serve metrics and keeps its
existing authentication/route contract. Prometheus accesses the internal service
port; public Cloudflare origins continue using the API port. Collector failures
do not change object lifecycle or authentication results.
An invalid enabled listener bind fails startup as a configuration error. Runtime
snapshot failures preserve the serving contract and mark cached state unavailable.

## Metric Families

| Family | Type | Labels | Meaning |
|---|---|---|---|
| `grove_build_info` | Gauge | version | Running binary version; value 1 |
| `grove_process_started_timestamp_seconds` | Gauge | none | Enabled metrics runtime start |
| `grove_http_requests_total` | Counter | surface, operation, outcome | Response created or handler interrupted |
| `grove_http_response_duration_seconds` | Histogram | surface, operation | Time to response creation, not body completion |
| `grove_http_in_flight` | Gauge | surface | Handlers awaiting response creation |
| `grove_management_operations_total` | Counter | surface, operation, outcome | Returned management command results, independent of HTTP status |
| `grove_management_duration_seconds` | Histogram | surface, operation | Returned command execution time, before history recording |
| `grove_history_write_errors_total` | Counter | stream | Best-effort invocation/security history writes that failed or timed out |
| `grove_transfer_bytes_total` | Counter | direction | Received DATA bytes or emitted response DATA bytes |
| `grove_transfers_total` | Counter | direction, outcome | Local byte-stream outcome |
| `grove_transfer_duration_seconds` | Histogram | direction, outcome | Receive/spool or response-body lifetime |
| `grove_transfer_in_flight` | Gauge | direction | Open observed byte streams |
| `grove_spool_slots` | Gauge | state | Used and maximum transfer slots |
| `grove_spool_reserved_bytes` | Gauge | none | Rounded admission reservations, not file lengths |
| `grove_spool_budget_bytes` | Gauge | none | Effective rounded reservation budget |
| `grove_spool_rejections_total` | Counter | reason | Slot or byte admission rejection |
| `grove_spool_file_bytes`, `grove_spool_files` | Gauge | none | Cached regular temporary file lengths/count, including crash remnants |
| `grove_db_pool_connections` | Gauge | pool, state | Total/idle/limit; not acquisition waiting time |
| `grove_provider_requests_total` | Counter | operation, outcome | One logical SDK send, including its internal retries |
| `grove_provider_duration_seconds` | Histogram | operation | SDK send time; GET ends when headers/body reader arrive |
| `grove_jobs_total` | Counter | worker, task, outcome | Completed, failed, skipped or interrupted job invocation |
| `grove_job_duration_seconds` | Histogram | worker, task | Invocation lifetime |
| `grove_job_items_total` | Counter | worker, task, outcome | Completed invocation's reported processed/failed/skipped items |
| `grove_job_in_flight` | Gauge | worker, task | Invocations currently running |
| `grove_job_last_completed_timestamp_seconds` | Gauge | worker, task | Last returned report, including errors |
| `grove_job_last_success_timestamp_seconds` | Gauge | worker, task | Last returned report with no recorded failures |
| `grove_worker_timeouts_total` | Counter | worker | Whole-pass deadline exceeded |
| `grove_retention_passes_total` | Counter | stream, outcome | Returned per-stream retention reports |
| `grove_retention_rows_deleted_total` | Counter | stream | Deletions reported by returned retention passes |
| `grove_retention_duration_seconds` | Histogram | stream | Per-stream pass duration |
| `grove_retention_last_success_timestamp_seconds` | Gauge | stream | Last error-free stream report |
| `grove_retention_oldest_remaining_timestamp_seconds` | Gauge | stream | Oldest still-eligible record, 0 when observed empty |
| `grove_objects`, `grove_objects_bytes` | Gauge | state | Location-backed active/reserved/purge-pending inventory |
| `grove_recovery_queue_objects` | Gauge | task | Persistent cleanup, expired completion and expired S3 upload queues |
| `grove_recovery_queue_oldest_timestamp_seconds` | Gauge | task | State-entry/lease-expiry timestamp, not the moving retry timestamp |
| `grove_recovery_queue_unknown_age_objects` | Gauge | task | Queue rows without a durable age anchor; not silently interpreted as new work |
| `grove_snapshot_available` | Gauge | source | Latest database/disk sample succeeded; initially 0 |
| `grove_snapshot_last_success_timestamp_seconds` | Gauge | source | Last successful sample, initially 0 |
| `grove_snapshot_errors_total` | Counter | source | Failed or timed-out samples |

Labels come from fixed operations, outcomes, workers, streams and states. Object
keys, file UUIDs, bucket/client IDs, accounts, endpoints and signed URLs are absent.
Counter values reset at process restart. Empty traffic is not zero latency or
proof of storage availability. No metrics include password/token values.
The `task` label avoids collision with Prometheus's scrape `job` label. Known
S3/transfer and rejection counters start at zero; latency has no samples until
an operation is observed.

## Completion Semantics

HTTP 200/206 does not confirm a completed download. The stream observes DATA
frames, EOS, errors and Drop. `success` means the expected Content-Length was
emitted; HTTP can stop polling without a final EOS poll. Empty bodies complete
immediately. Early EOS with fewer bytes is `incomplete`. Errors and interrupted
streams are separate outcomes. Emitted
bytes do not prove receipt at the client; NoteGate/browser observations are the
end-to-end authority. Receive success means the declared bytes reached the spool,
not that checksum validation, provider commit or metadata publication succeeded.

Provider `not_found` separates SDK 404 from other errors; idempotent cleanup can
treat that response as lifecycle success. SDK retries are not separate counters.
Presigned vendor URL issuance is not an observed provider transfer.

Management operations use the closed command catalog and server-selected surface.
Results distinguish success, denied, error and unknown commit outcome. MCP can
return HTTP 200 for a failed command; HTTP and command counters remain separate.
Only returned operations are counted here; cancellation before a result is not
fabricated as a completed command. Transactional audit records remain separate
from best-effort invocation/security history write failures.

An empty successful job scan counts as success. Claim contention is skipped,
not a failure. Physical/metadata errors count as job failures even when the
worker continues to other candidates. Whole-pass cancellation records an
interrupted invocation, not success. Item/deletion counts are completed reports;
partial work before cancellation or response loss is not an exact database ledger.

## State Sampling

The sampler runs every 60 seconds. Database and spool scans each have a two-second
deadline. Database reads use the existing maintenance pool, not serving connections.
The database snapshot is a read-only transaction with a one-second PostgreSQL
statement timeout and a 250-millisecond lock timeout, scoped to that transaction.
The scrape route reads only the in-memory registry. A failed sample leaves prior
values and timestamps intact and sets availability to 0. Freshness is
`time() - grove_snapshot_last_success_timestamp_seconds`, not a growing age gauge.

Object bytes are registered metadata/location observations, not provider billing
or physical free capacity. Terminal files with no locations are not cleanup
backlog. Recovery queues include retry backoff; unexpired completion ownership
and normal open uploads are excluded. Queue ages use stable state-entry/expiry
timestamps, so retries cannot keep old failed work looking new. Deleted files use
`deleted_at`; reclaimed files use their expired write lease. Missing durable age
anchors are counted separately instead of using file creation time as a guess.

Database gauges are shared state exported per Pod. Dashboards use `max`, not
`sum`, across replicas for inventory/queues. Database series are filtered by that
same target's successful, fresh snapshot before aggregation, so one replica's
stale cache cannot borrow another replica's healthy timestamp. Pool/spool gauges and counters are
process-local. The disk sample sums regular file logical lengths; it is not
allocated filesystem blocks or free node space. Node filesystem monitoring remains
separate. SQL pool occupancy does not measure acquisition or individual query time.

## Verification

Tests cover fixed labels, byte counts, EOS/partial/error/drop, balanced in-flight
gauges, stale snapshots, spool permit release, job failures and virtual elapsed
time. PostgreSQL tests cover queue eligibility and terminal-history exclusion.
The metrics listener works without management cookies/tokens and exposes no data
or metrics route on the public API. The disposable E2E checks presigned
PUT/GET/Range/HEAD/DELETE observations and scrape availability after DB shutdown.
Promtool checks dashboard expressions and alert behavior. Deployment artifacts and
activation guidance are in [observability](../../deploy/observability/README.md).
