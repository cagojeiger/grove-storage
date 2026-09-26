# Grove Storage Console

The console uses English labels and messages, `lang="en"`, and `en-US` number
formatting. The UI language is independent of the browser locale.

## Browser Preview

Run `npm run build` and `node scripts/preview.mjs` in this directory. The printed
HTTP address opens in browsers that do not trust a local test certificate. It serves
sample registry data in memory; edits reset when the preview server stops. It does
not connect to PostgreSQL or validate S3 credentials. Use the isolated HTTPS fixture
below to test authentication and the real API.

The sample preview starts signed out. Use `qwer1234` in **Account token** for the local Root preview.
This fixed demo token and process-wide sample session are only for the loopback
preview; production uses DB-backed User credentials and browser sessions.
The preview includes Admin/Root-only Accounts with sample Users and token lifecycle.
Activity returns an empty sample history; Settings lists the current sample session.
Use the real fixture below to verify persisted history and independent sessions.
Its setup/recovery entry accepts the same demo master token and starts initialized.
The real fixture below verifies first-Admin bootstrap against an empty database.

| Implemented | Follow-up |
|---|---|
| Personal User token login/logout, current role, session restore and 401 handling | Production authentication/ingress verification |
| Master setup/recovery, Admin-only Users, roles, enable/disable/delete, token issue/revoke | Account rename requires a separate backend contract |
| Storage list/detail, S3 create/replace/delete, legacy FS read-only detail, conflict guards | Standalone draft connection test |
| Clients create/detail/delete, S3 credential issue/revoke with one-time secrets | Runtime Client Logs; Native keys remain API/CLI/MCP-only |
| API readiness, client count, per-storage/client usage and daily snapshot history | Server-paged large usage histories |
| Activity: audit, command history, security events; scoped cursor paging and event details | Server-side actor/resource/date filters |
| Settings: current account/role, own sessions, confirmed session revocation | Overall navigation and UX review |
| Registration/replacement checks and saved S3 **Test connection**, shared with CLI/MCP | Standalone draft test; sample preview performs no probes and returns unavailable |
| System/light/dark, mobile/tablet/desktop | Production static hosting and TLS ingress |

## Isolated Local Console

From the repository root, build `filegate` and `gscli` with `cargo build --bin filegate --bin gscli --locked`.
On macOS use `DEVELOPER_DIR=/Library/Developer/CommandLineTools` if required by the Rust linker.

```sh
cd frontend/web
nvm use
npm ci
npx playwright install chromium
npm run build
npm run lint
npm test
cd ../..
```

The HTTPS fixture always uses a disposable MinIO backend:

```sh
python3 -m venv /tmp/grove-s3-sdk
/tmp/grove-s3-sdk/bin/pip install boto3==1.43.99
/tmp/grove-s3-sdk/bin/python -B -u scripts/e2e-console.py
# Keep the same disposable fixture open for manual browser checks:
/tmp/grove-s3-sdk/bin/python -B -u scripts/e2e-console.py --serve
```

CI runs the same fixture; `--with-minio` remains a compatibility alias. It creates and removes its own MinIO container and volume.

The fixture needs Docker, Python 3, Node 22.13+ (22.x) or 24+ and OpenSSL. It creates a disposable
PostgreSQL database, API and HTTPS Vite server. `--serve` prints the URL and a local
mode-0600 token file. The certificate is self-signed and scoped to this local fixture;
the browser may show a trust warning. Ctrl-C or SIGTERM cleans up the fixture.
No production endpoint or credentials are used. The initial overview is empty.

The project development and CI baseline is Node 24 (`.nvmrc`); any Node version
manager can select it. `nvm use` applies when using nvm. Frontend lint/build/browser
checks run independently of Rust CI. The real HTTPS API check runs with the Rust
binaries in the backend job. TypeScript remains pinned to 6.0.3.

## Existing Development API

```sh
GROVE_DEV_API=http://127.0.0.1:8080 \
GROVE_DEV_TLS_KEY=/absolute/path/key.pem \
GROVE_DEV_TLS_CERT=/absolute/path/cert.pem npm run dev
```

Set the API's `FILEGATE_CONSOLE_ORIGIN` to the exact HTTPS Vite origin. Open
**Setup & recovery** on the sign-in screen and enter the configured Root
token to issue the first Admin's User token. Save it, then sign in separately.
Recovery requires an existing active Admin UUID and explicit confirmation; it replaces
that Admin's tokens and revokes its sessions. The browser fixture verifies both flows
against a disposable DB; `--serve` pre-creates a User token for manual checks.
Root also signs in through the normal **Account token** form. Its separate,
30-minute console session supports Accounts and resource management. Accounts
shows Root as config-owned and protected. See [ADR 011](../../docs/adr/011-root-and-accounts.md)
for `GROVE_ROOT_*` configuration, legacy compatibility and rotation.
Clients supports registration, guarded deletion and S3 credentials through
the same command API as CLI/MCP. S3 secrets are shown once without browser persistence.
Native API, CLI, MCP and DB compatibility remains unchanged; the console offers
S3 credentials only. Activity uses the existing scoped management history APIs.
Settings revokes browser sessions without revoking their source account token.
Legacy operator tokens do not log
into the console. The default UI port is 5173; an occupied port fails instead of
silently changing the allowed origin. Override with `-- --port PORT` and update the origin.

The [Vite HTTPS/proxy configuration](https://vite.dev/config/server-options) retains
the browser Origin. API routes and `/readyz` are proxied; browser requests use
same-origin cookies. Token/provider secrets never belong in `VITE_*` variables.
For release hosting, use a dedicated HTTPS console host. Mount `dist` at
`/api/admin/console/` and proxy only `/api/admin/identity/v1` (including its subpaths),
`/api/admin/console-commands/v1`, and
`/readyz`. Return 404 for all other paths. Serve S3, relay and uploaded files on a
different host: uploaded HTML running on the console origin could act with the
administrator's cookies. This does not require a separate backend process.

Apply the production defaults in `security-headers.mjs` to the HTML response at the
static host. The built sample preview uses these headers. Vite development adds
inline scripts/styles and websockets for React refresh and HMR; use the default
policy for release hosting. Configure HSTS at the production TLS terminator.
The backend image and production ingress are not configured by this frontend.
See [browser security](../../docs/spec/07-browser-security.md) for deployment checks.

## Verification

TypeScript is pinned to 6.0.3. ESLint 10 and typescript-eslint 8 use the
[recommended type-aware rules](https://typescript-eslint.io/getting-started/typed-linting/)
for application, test and configuration TypeScript. React Hooks order and dependency
checks are errors. JavaScript test/configuration files use the recommended ESLint
rules; generated build, browser reports and local TLS files are excluded.
`npm run lint` fails on warnings as well as errors and runs in CI before the build.

| Test | Boundary |
|---|---|
| `tests/api.spec.ts` | Transport options, cancellation signal, error sanitization, formatting |
| `tests/commands.spec.ts` | Shared command envelope, outcome certainty, no automatic mutation retries |
| `tests/permissions.spec.ts` | Reader controls, live demotion and form removal |
| `tests/console.spec.ts` | Mock API state/error handling, 320/390/768/1024/1440px, themes, screenshots |
| `tests/browser-security.spec.ts` | Built assets under CSP, blocked inline scripts/external connections/iframe embedding |
| `tests/storage-model.spec.ts` | Exact capacity conversion, S3 payloads and FS rejection, sanitized errors |
| `tests/storage-crud.spec.ts` | All S3 options, complete replacement, legacy FS editing guard, ID confirmation, navigation |
| `tests/storage-safety.spec.ts` | 409, 401, lost response, secret clearing, duplicate submit guard |
| `tests/storage-layout.spec.ts` | List/detail/editor across five widths and both themes; focus restoration |
| `tests/access.spec.ts` | User creation, roles, last-Admin conflict, one-time token issuance/revocation and paging |
| `tests/access-safety.spec.ts` | Role loss, expired sessions, duplicate submission and issued-token account correlation |
| `tests/access-layout.spec.ts` | Account detail and token dialog across five widths and both themes |
| `tests/master-setup.spec.ts` | Separate master/User sessions, initial setup, targeted recovery and unknown outcomes |
| `tests/clients.spec.ts`, `client-safety.spec.ts`, `client-layout.spec.ts` | S3-only key controls, client lifecycle, conflicts, secret handling and responsive layouts |
| `tests/maintenance.spec.ts`, `maintenance-layout.spec.ts` | Scoped history, bigint cursors, demotion, session revocation, unknown outcomes and responsive layouts |
| `tests/usage-history.spec.ts` | Snapshot dates/counts, range validation, empty/error/401, progressive rendering and responsive tables |
| `tests/resource-navigation.spec.ts` | Bounded paging, list state across detail/reload/deletion, authoritative Client assignment and overview highlighting |
| `tests/resource-layout.spec.ts` | Multi-resource topology and paged lists at phone/tablet/desktop widths in both themes |
| `tests/overview-connections.spec.ts` | Six/seven folding boundary, aggregate totals, curved paths, selection pinning, bounded grouped browser and failure states |
| `tests/overview-built.spec.ts` | Production-built unselected connections on initial mount, reload and navigation return |
| `tests/live.mjs` via Python fixture | Real HTTPS cookie attributes, CSRF, reload/logout, storage usage, expiry and token revocation |
| `tests/live-storages.mjs` via Python fixture | UI MinIO lifecycle, concurrent client reference deletion guard, pending-file address change guard |
| `tests/live-permissions.mjs` via Python fixture | Real role demotion, Reader enforcement, named User token login and Console audit |
| `tests/live-access.mjs` via Python fixture | Real first-Admin bootstrap, User lifecycle, token use/revocation, last-Admin protection and targeted master recovery |
| `tests/live-clients.mjs` via Python fixture | Real client/S3 credential lifecycle and pending-file deletion guard |
| `tests/live-maintenance.mjs` via Python fixture | Real history views and revocation of another/current browser session with independent cookies |
| `tests/live-usage.mjs` via Python fixture | Real usage API and date range against seeded disposable DB snapshots; deleted resource history retained |
| `tests/live-resources.mjs` via Python fixture | Real Client assignment highlighting and resource list return state |

Expiry is injected into the isolated database; the test does not wait eight hours.
Run `npm run build` before `npm test`: the browser security suite serves `dist` on
loopback port 5180, alongside the existing Vite test server on 5179. Login regression
tests verify that 307/308 redirects do not forward token bodies.
Real storage registration uses a disposable S3 backend. Mutation requests are not retried automatically; unknown
outcomes require a fresh read. S3 secrets are cleared at submission and never stored
in browser storage or the query/mutation cache. Capacity input is limited to exact
JSON integers (0 through 2^53-1 bytes); the backend's wider i64 contract is unchanged.
Issued management tokens stay in component state only. Closing requires an explicit
saved-token acknowledgement; the token is then discarded. Access search filters
loaded pages; **Load more** fetches the next server page. User identity actions
use console-only APIs, separate from resource commands available to CLI/MCP.
