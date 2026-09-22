# Storage Registry

Status: domain and use cases implemented; production adapters pending.

## Operations

| Operation | Contract |
|---|---|
| Register | Validate values and credentials, check duplicate, probe, protect, atomic insert |
| Get | Return public configuration and revision; protected credentials stay internal |
| Replace | Expected revision, fresh credentials, probe, protect, atomic guarded replacement |
| Delete | Atomic reference check and deletion; absent storage is an idempotent success |

```text
validated input
      |
      v
registry use case
      +--> read-only provider probe
      +--> identity-bound credential protection
      +--> atomic repository operation
```

| Condition | Result |
|---|---|
| Invalid ID, endpoint, capacity, region or bucket | Domain construction fails |
| Empty credentials | InvalidCredentials before I/O |
| Existing registration | AlreadyExists; existing row preserved |
| Probe failure | ProbeFailed; no database write |
| Protection failure | ProtectionFailed; no database write |
| Persistence failure | PersistenceFailed; outcome may be unknown; no automatic retry |
| Stale revision | ConcurrentChange |
| Missing replacement target | NotFound |
| Client/file/upload/cleanup reference | Delete and physical retargeting return InUse |
| Public endpoint, capacity or relay-mode change | Allowed with references, subject to revision check |

Physical identity includes endpoint, region, bucket and path-style routing. Changes
are conservatively guarded even when two addresses might refer to the same data.
Capacity is an observational baseline; zero means unlimited, not zero storage.
`force_relay` is configuration only here; a future application adapter must verify
that the relay service is configured before enabling it.

Provider credential rotation is a replacement with an unchanged physical target.
The probe is read-only and does not prove all read/write/delete permissions.
Endpoint syntax validation is not an SSRF policy; the production adapter must set
deployment-appropriate destination and redirect restrictions.

## Tests

| Suite | Boundary |
|---|---|
| domain/tests/values.rs | Legacy slug bounds, endpoint syntax, capacity, required fields |
| domain/tests/policy.rs | Independent reference types, retargeting vs metadata changes |
| registry/tests/registration.rs | Secret redaction, injected failures, early rejection |
| registry/tests/lifecycle.rs | Safe mutation, idempotent deletion, stale revisions |
| registry/tests/interleavings.rs | Reference arrival during probe and conflicting writes |

The test protector emits a marker, not ciphertext. It proves orchestration only.
The memory repository serializes operations under a mutex. The PostgreSQL adapter
must independently establish the same contract with real database tests.
