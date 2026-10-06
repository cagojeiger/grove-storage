# spec 06: Management Console

Status: implemented and verified locally against Rust, PostgreSQL and MinIO.
Release, production hosting and operational migration remain separate gates.
Authentication follows [spec 11](11-local-management-auth.md); browser hosting
follows [spec 07](07-browser-security.md).

## Scope and Boundaries

```text
Console -- password session --> Identity API --> Accounts / tokens / history
        -- password session --> Resource commands --+
CLI / MCP -- account API token --> Resource commands +--> PostgreSQL / S3 probe

Client -- service credentials --> Native / S3 file APIs --> External S3
```

| Boundary | Contract |
|---|---|
| Storage backend | External S3 only; migration 0017 rejects remaining FS rows before schema changes |
| Local disk | Temporary transfer spooling, separate from persistent object storage |
| Resource commands | Console, CLI and MCP share the [command catalog](09-management-commands.md) |
| Identity and history | Password-session APIs under `/api/admin/identity/v1`; current role and CSRF checks |
| Service keys | Console offers S3 credentials; Native keys remain supported by API/CLI/MCP |
| Account authentication | Username/password; first Admin and recovery use server-local commands |
| Root/Master | Historical design only; no current console login or recovery route |

## Navigation

```text
Grove Storage
├── Overview
│   └── Usage history
├── Resources
│   ├── Storage       list / detail / Add storage / Edit storage
│   │   └── Settings + metadata / Usage / Test connection
│   └── Clients       list / detail / Create client
│       └── Settings + metadata / Stored usage / S3 Credentials
├── Management
│   ├── Accounts      Admin only: list / detail / Create account
│   │   └── Password setup / Management API tokens / Danger zone
│   └── Activity
│       └── Audit log / Command history / Security events
├── Developer
│   └── API docs      Swagger UI: S3 / Management / Native compatibility
└── Sidebar identity
    ├── My account    direct link: Profile / Security / API tokens / Sessions
    └── Sign out     separate icon action

App bar: breadcrumb / theme; mobile: navigation drawer trigger
Entry: Sign in / one-time Set password / operator recovery help
```

| View | Data and operations |
|---|---|
| Overview | Client and Storage counts, stored files/data, configured capacity and separate reservations/cleanup |
| Connections | Clients left, Grove center, Storage right; stacked on narrow screens; curves show configured routes, not observed traffic |
| Map bounds | Up to six entries per side; seven or more shows five with a View all link and remaining count; selection pins the assigned Storage |
| Remaining resources | View all opens the resource list; search and local pagination; resolve only the visible Client page |
| Storage list | S3 settings, configured capacity and Grove-managed usage; row opens detail |
| Storage editor | Dedicated create/edit route; full-spec replacement re-enters the secret; saved data refreshes without redirecting a user who left the editor |
| Clients | ID, assigned Storage, stored files/data, metadata and one-time S3 credentials |
| Accounts | Server-side search, role/status filters and cursor paging; create, rename, change role, enable/disable/delete |
| Activity | Committed changes, command outcomes and security events; actor/token filters and event details |
| My account | Display-name edit, own tokens and sessions; revoking the current session signs out |
| API docs | Read-only Swagger UI and OpenAPI 3.1 JSON; public integration contracts, not browser-internal identity routes |

Reader/Writer see their own management history; Admin sees installation history.
Security events and account administration require Admin. Client file-request logs
are a future contract, separate from management Activity.

## Presentation

| Concern | Implementation |
|---|---|
| Foundation | React, TypeScript, MUI Material, MUI X Community Data Grid and Charts |
| Typography | System stack in `src/template/shared-theme/themePrimitives.ts`; template type scale; no web font download |
| Theme | System/light/dark; green brand scale, zero letter spacing; official shared-theme component overrides |
| Layout and controls | Official Dashboard and Sign-in templates; MUI Stack/Grid, Drawer/AppBar, Data Grid, dialogs and native form constraints |
| Custom surface | Overview measured connection geometry; domain composition and responsive sizing through MUI `sx`; no standalone CSS |
| Language | English labels, `lang=en`, `en-US` numbers; B/KiB/MiB/GiB/TiB display |
| Dialogs | Responsive MUI Dialog, scrollable content and visible actions; one-time secrets require saved confirmation |

## Lists and Usage

| Surface | Pagination / meaning |
|---|---|
| Storage and Clients | Data Grid; local paging, default 20, choices 20/50/100; full registry list comes from the API |
| Search and sort | Resource ID; natural ID order; filter/order/size changes reset page |
| Return state | Hash query preserves list state across detail, reload and Back; deletion clamps the page |
| Client lookups | Visible list page only, at most 100; Overview at most six |
| Accounts | Server search/role/status filters and cursor navigation; filters and page history survive reload |
| Activity | Data Grid previous/next; 50-row server cursor pages, unknown total until the last page |
| Tokens, sessions | Server cursor paging through Load more |
| Usage history | `usage.history`, 1-3650 days, default 90; UTC snapshots; local Data Grid paging default 20, choices 20/50/100 |
| Usage chart | Recorded stored files/data; absent days remain gaps; deleted resource IDs retain historical rows |

Configured capacity is an operator-set allocation, not provider free space.
Usage counts Grove-managed objects, upload reservations and pending cleanup.
Daily snapshots are recorded stock, not transfer logs or exact historical billing.
Missing usage stays unavailable; a successful empty result means zero stored files.

## Resource Forms and Safety

| Operation | Contract |
|---|---|
| Add/Edit storage | ID, endpoint, optional public endpoint, region, bucket, access/secret keys, path-style, relay, capacity |
| Capacity | B/GiB/TiB input converts exactly to nonnegative integer bytes through 2^53-1; server i64 contract is unchanged |
| Replace | Full settings plus re-entered secret; referenced address changes return 409; supported capacity/credential updates remain available |
| Delete storage/client | Typed ID confirmation; server rechecks references, including pending files and cleanup |
| Resource metadata | Separate string-valued JSON object, normalized size at most 8 KiB; full replacement, `{}` clears; displayed with basic settings |
| Metadata meaning | Operator labels/descriptions, independent of routing, credentials and object metadata; values stay out of management audit payloads |
| S3 credential | One-time secret; confirm saved before closing; copy success or manual-copy guidance; revoke names key and Client |
| Pending mutation | Duplicate submission blocked; entered provider secrets cleared at submission |
| Unknown outcome | Stop resubmission and re-read authoritative state; no automatic mutation retries |
| 401 / 403 | Clear private state or refresh current permissions; server remains the authorization boundary |

## Connection Test

| Item | Contract |
|---|---|
| Entry | Storage detail Test connection; `gscli storage test ID`; MCP `storage.test` |
| Permission | Reader/Writer/Admin resource-read permission; identity rechecked after probe |
| Probe | Saved credentials, internal S3 `HeadBucket` and `ListMultipartUploads`; same baseline as registration |
| Result | `{id,state:"ok"}`; 503 for provider/decryption/timeout failure; private diagnostics excluded; UI shows local observation time |
| Limits | Does not prove object PUT/GET/DELETE permission, public endpoint reachability or browser CORS |
| Execution | Explicit action; 10-second timeout outside identity transaction; one pending UI request, no automatic UI retry |
| Freshness | Recheck saved settings; changed target returns 409, deleted target 404; refresh/navigation clears displayed result |
| History | Command invocation only; registry unchanged, so no storage-change audit |
| Draft | Registration/replacement runs pre-save checks; standalone draft Test connection remains deferred |

`readyz` and remote `status` describe API/registry state, not provider health.
Server-local `filegate status` probes registered backends using server settings.

## Account Workflows

| Flow | Contract |
|---|---|
| First Admin | `filegate account init` in a server terminal, then normal password sign-in |
| Create account | Admin re-enters current password; username, display name, role and setup link created atomically |
| Initial password | Recipient consumes one-time fragment link, sets password, then signs in |
| Reissue setup | Active, non-deleted account without a password; reserved username is read-only |
| Password configured | Initial setup action disabled; use own password change or operator recovery |
| Recovery | `filegate account recover`; same account ID, sessions and management tokens revoked |
| Management token | Label and 1-90 day expiry; current-password reauthentication; masked one-time display with reveal/copy and saved acknowledgement |
| Token boundary | Named account token authorizes resource commands; console login uses a password |
| Role safety | Last usable Admin protected; disable/delete confirms account name; self-actions disclose session loss |

Account tokens, Client service credentials and provider secrets remain separate.
Changing a password revokes browser sessions and preserves management API tokens.
Full authentication and transaction contracts live in [spec 11](11-local-management-auth.md).

## Verification and Delivery

| Evidence | Scope |
|---|---|
| `npm test` | Mock contracts, responsive layouts, themes, navigation, error/permission states and production CSP |
| `scripts/e2e-console.py` | Real Rust/PostgreSQL/MinIO, HTTPS cookies/CSRF, account setup/recovery, resource lifecycle, transfer bytes, usage, metadata and audit |
| `scripts/e2e-cli.py`, `scripts/e2e-mcp.py` | Shared catalog, role/revocation, reference guards, audit attribution and secret-free logs |
| `scripts/e2e-s3.py` | Real MinIO objects, supported SDK/presigned/multipart contracts and provider outage/restart |
| Sample preview | In-memory illustration only; no PostgreSQL, credential validation or S3 probes |
| Production | Separate static-host/TLS/proxy/CSP checks, migration inventory and rollout |

The executable procedures are in the [frontend README](../../frontend/web/README.md).
Production serves the console and identity/resource-command APIs on a dedicated
HTTPS origin; uploaded files, S3 and relay use a different host. This does not
require another backend process. OIDC, runtime Client Logs, server-paged resource
summaries and automated placement remain follow-up work.
