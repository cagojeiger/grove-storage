# Grove Storage

Standalone storage control plane with PostgreSQL metadata and external S3 storage.

**Status: contract-first foundation. No runnable server or production adapters yet.**

```text
crates/
  domain/       validated values and storage lifecycle policy
  registry/     registration, lookup, replacement and safe deletion
docs/
  adr/          architecture decisions
  spec/         feature contracts and test boundaries
```

## Build And Test

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

The current suite runs without PostgreSQL, Docker, S3 credentials or HTTP servers.
Tokio is a test-only dependency of the registry crate. Public registry ports use
static dispatch; runtime-specific task spawning is left to future adapters.

## Roadmap

| Stage | Scope | Status |
|---|---|---|
| Feature foundation | Storage values, registration and mutation contracts, isolated tests | Implemented |
| PostgreSQL adapter | Atomic reference guards, revision conflicts, real concurrency tests | Next |
| S3 and credential adapters | Read-only probe, encrypted secrets, failure injection | Planned |
| Management surfaces | One API, CLI and console feature parity, administrator authentication | Planned |
| File lifecycle | Import existing object identity, upload/recovery contracts | Planned |
| Operating migration | Inventory, rehearsal, cutover and rollback policy | Planned |
| Storage nodes | Mounted filesystems and joining agents | Phase 2 |

FileGate continues operating separately. This repository neither changes its DB
nor migrates existing objects. Branding assets and console prototypes remain in
the reference project until the relevant UI stage.

- [Architecture boundary](docs/adr/001-feature-first.md)
- [Storage contract](docs/spec/001-storage-registry.md)
- [Migration constraints](docs/migration.md)
