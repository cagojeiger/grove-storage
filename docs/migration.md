# FileGate Migration Constraints

Status: planning constraints from source inspection; production inventory pending.

| Item | Initial rule |
|---|---|
| File IDs, client IDs and storage IDs | Preserve existing identity |
| S3 logical keys and physical object paths | Preserve mappings; metadata migration first |
| External buckets | Reuse where possible; no implicit object move |
| Encrypted credentials | Preserve key IDs, root secrets, AAD and derivation labels until explicit conversion |
| Current crypto labels | `filegate/enc/storage-secret/v1` and relay derivation remain compatibility data |
| Pending uploads and cleanup | Drain or import recovery state under a tested procedure |
| Old/new writers and reconcilers | One authoritative writer/cleanup owner during cutover |
| Rollback after new writes | Requires reconciliation; not simply an old DB restore |

The initial domain models a storage registry, not the complete target DB schema.
Repository revisions are a new concurrency contract and need a migration strategy.
Do not run the new product against the production database until the adapter,
schema, legacy cryptography and migration rehearsal are validated.
