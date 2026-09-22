# Grove Storage

- Build independently tested feature contracts before server wiring.
- Keep `grove-domain` independent of runtimes, databases, and provider SDKs.
- Feature crates own use cases and ports; adapters own I/O and atomic enforcement.
- Preserve existing work. Keep changes scoped and document behavior changes.
- Tests are split by behavior. Fakes do not prove PostgreSQL locking or S3 compatibility.
- Preserve legacy IDs, object paths, and cryptographic derivation for migration until an explicit conversion exists.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- Commits use `type: Imperative subject` (feat/fix/docs/refactor/test/chore/style).
