# ADR 011: Root and Accounts

Status: local implementation; deployment is a separate step.

```text
Config-owned Root            Accounts: Config / Protected
  +-- Root console session   30 minutes, generation-fenced
  +-- Setup/recovery session 10 minutes, single-use

Database-owned Users         Accounts: Admin / Writer / Reader
  +-- Named User tokens      Console / CLI / MCP
  +-- Browser sessions       Bound to the issuing token

Clients                      Native / S3 service credentials
```

| Boundary | Contract |
|---|---|
| Root | One config-owned principal, shown in Accounts; no mutable User row |
| Root authority | Full console resource/account management; existing reference/deletion guards apply |
| Root authentication | Token exchange at the normal sign-in; separate session table and hash domain |
| Setup/recovery | Explicit reauthentication using the config token; existing setup cookies retain their restricted authority |
| User authority | Admin manages Accounts; Writer changes resources and service keys; Reader views resources |
| Machine access | User tokens access CLI/MCP; Root tokens and console cookies stay outside Bearer authentication |
| Rotation | Increasing the config generation revokes Root/setup sessions and fences older server configuration |
| Protection | Root has no role-change, disable, delete or User-token issuance endpoint; last active Admin protection remains |
| Audit | Root operations retain the stable config-principal storage name `master`, with session/request IDs; new Root session actions use `root.session.*` |

## Configuration

| Setting | Value |
|---|---|
| `GROVE_ROOT_TOKEN` | `gsrt_` followed by 64 lowercase hex digits |
| `GROVE_ROOT_GENERATION` | Positive integer, increased on token rotation |
| Legacy compatibility | Existing `FILEGATE_MASTER_TOKEN` / `FILEGATE_MASTER_GENERATION`, token prefixes, hash domains and setup endpoints remain supported |
| Ambiguity | Configure one environment-variable pair; mixed pairs fail startup |

Migration `0014_root_sessions.sql` adds only the separate Root session table.
User accounts, credentials, existing sessions, audit snapshots and data-plane tables retain their existing data.
Apply migrations before starting the updated API. Stop older writers during the coordinated upgrade.

## Console

| View | Contents |
|---|---|
| Accounts | Protected Root entry plus paged User accounts and named tokens |
| Root detail | Configuration status and link to Setup & recovery |
| Setup & recovery | First Admin creation or targeted Admin token replacement after explicit config-token authentication |
| First installation | Use Setup & recovery to create the first Admin before adding other accounts |

The `#accounts` route replaces the visible Access name; old `#access/*` links remain usable.
