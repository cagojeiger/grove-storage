# Time Boundaries and Deterministic Tests

Time is a dependency, but it has three different authorities. Do not replace
database time with an application clock or use wall time to measure deadlines.

| Responsibility | Production authority | Test control |
|---|---|---|
| SigV4 validation, presigning, cookie age, token expiry inputs, local spool age | `filegate_core::time::Clock` / `SystemClock` | Inject `FixedClock` through `AppState`, `S3ClientCache`, or `s3_connect_with_clock`; spool cleanup also accepts an explicit timestamp |
| Timeouts, heartbeat, object/management worker ticks, SDK sleep/retry, elapsed logging | Tokio monotonic clock | Pause/advance the Tokio scheduler; timeout/heartbeat helpers accept durations |
| Sessions, setup links, authentication budgets, leases, lifecycle GC, retention, resource timestamps and usage history | PostgreSQL | Replace the two clock functions only inside each disposable SQLx test database |

## Database Contract

Migration `0023_time_boundaries.sql` adds `grove_time.transaction_now()` and
`grove_time.wall_now()`. Production implementations retain `CURRENT_TIMESTAMP`
(transaction-stable) and `clock_timestamp()` (wall time), respectively. Existing
timestamp defaults and Rust lifecycle queries use these functions. No business
columns or stored object keys are rewritten. New object keys use the inserted
file's database timestamp, without an extra database round trip.

The test-only helper is `backend/crates/db/tests/support/time.rs`. Its clock table
and replacement functions exist only in disposable test databases. There is no
production time override via environment variables, requests, or pooled session
settings. Never run this helper against an operational database.
The helper records clock versions against PostgreSQL transaction-start time so
that advancing virtual wall time does not advance an already-open transaction.

Historical migration fixtures insert historical rows directly before upgrading;
current writers require the latest migration. New migrations adding time-based
defaults must use the common functions. Keep timezone behavior explicit when
deriving calendar dates; usage snapshot creation uses the UTC date.

## Coverage

- SigV4: exact expiry and future/past skew boundaries, including subsecond edges.
- SDK GET, PUT and multipart part URLs: fixed signing time and TTL limits.
- SDK network request signing: both injected local time and a fixed provider
  `Date` header (the SDK can otherwise adjust for observed clock skew).
- Sessions and parent API tokens: immediately before and at expiration.
- Password setup: exact expiration rejects completion without updating a hash.
- Authentication window: exhaustion before the minute boundary and reset at it.
- Write lease: expiry prevents renewal; reclaim rechecks a renewed lease.
- Upload ownership: receive past TTL, blocked reclaim and renewal on completion,
  without the previous real 1.1-second wait.
- Retention and usage history: exact cutoff and calendar-date transitions.
- Database clocks: transaction time stays frozen while virtual wall time advances.
- Heartbeat: first tick, ownership loss, completed operations and no catch-up burst
  after a delayed renewal.
- Stream spool: idle timeout and a refreshed idle budget for each received chunk.
- Shutdown: a single total budget covers worker draining and pool closure.
- Password hashing admission: queue timeout and capacity released before timeout.
- Temporary files: exact age threshold and future modification timestamps.

`tokio/test-util` is a dev dependency feature. Real network, filesystem and SQL
locking tests still use actual I/O; pausing Tokio does not pause PostgreSQL or
filesystem modification times.

Run with a disposable PostgreSQL `DATABASE_URL`:

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Limits

Deterministic clocks verify covered time boundaries, not every failure ordering.
They do not themselves impose an S3 operation deadline, guarantee reconciliation
fairness, or prove provider response-loss handling. Those remain separate I/O and
recovery contracts. Avoid a blanket short deadline on large streaming transfers.
