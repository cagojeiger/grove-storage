# Local Authentication Verification Record

Recorded source: `352213d026d84dfe2e5c70d86c26f1bc459840a3`,
`docs/spec/11-local-management-auth.md`. Separated on 2026-10-06.

These are historical implementation-stage results copied from that document.
The original entries do not identify a tested revision or execution date for
each stage. Their counts describe those stages, not the latest candidate.
This documentation change does not rerun these suites or establish release,
deployment or production migration status.

The current authentication contract is [spec 11](../spec/11-local-management-auth.md).
Test locations are in [source layout](source-layout.md#검증-위치).
Candidate, release and migration evidence belongs to
[production readiness](production-readiness.md).

## Historical Delivery Stages

| Stage | Required evidence | State |
|---|---|---|
| 1. Password foundation | Policy/hash tests, initialize/recover atomicity and concurrency tests | Verified locally |
| 2. Session cutover | Login, expiry, CSRF, generation race, password-change and revocation tests | Verified locally |
| 3. Account UX | Setup-once flow, role/last-admin guards, personal tokens/sessions | Verified locally |
| 4. Machine interfaces | CLI/MCP parity, role change, expiry/revoke, identity API denial | Verified locally |
| 5. Cleanup and release readiness | Remove old Root/Master flow, align docs, full regression and responsive browser tests | Local verification; migration 0024 implements historical authentication cleanup |

### Foundation Evidence

| Check | Result |
|---|---|
| Password unit tests | 3 passed: canonical username, policy, salted Argon2id/Unicode verification |
| Password DB tests | 7 passed: initialize race, rollback, targeted revocation, login-name guards |
| Local account service tests | 2 passed with real hashes and PostgreSQL |
| Local command parser | Passed; recovery requires confirmation and password arguments are rejected |
| Existing DB/management service suites | Passed against isolated PostgreSQL 17 |
| Server binary | Interactive init/recover passed; same account ID; secret-free local audit; non-TTY rejected |
| Static checks | Rustfmt, Clippy with warnings denied, diff whitespace check passed |

### Session Evidence

| Check | Result |
|---|---|
| Password login and change API | Five PostgreSQL-backed tests passed: role freshness, recovery fencing, CSRF, revocation and rollback |
| Existing DB/API/management suites | Passed against fresh isolated PostgreSQL 17; migration upgrade preserves existing session data |
| Browser regression | 214 Playwright tests passed; password form checked at phone and desktop widths in light/dark |
| Real HTTPS fixture | Disposable PostgreSQL and MinIO: local recovery, password login, password change, re-login and Resource workflows passed |
| Static checks | Rustfmt, Clippy with warnings denied, frontend lint and production build passed |

### Setup Link Evidence

| Check | Result |
|---|---|
| DB setup tests | Four PostgreSQL-backed tests passed: name reservation, replacement, expiry/deletion/recovery, single winner and audit rollback |
| Console API tests | Two PostgreSQL-backed tests passed: Admin password session and reauthentication, CSRF/Origin, single-use completion and login |
| Browser regression | 220 Playwright tests passed, including one-time link display, fragment removal and responsive setup form |
| Real HTTPS fixture | Disposable PostgreSQL and MinIO: Admin issued a link, another browser set a password and signed in as Reader |

### Personal Token Evidence

| Check | Result |
|---|---|
| DB/API ownership tests | Self-only list/revoke, password-session gate, reauthentication and CSRF passed against PostgreSQL 17 |
| Browser regression | 225 Playwright tests passed, including personal-token lifecycle and phone/desktop light/dark layouts |
| Real HTTPS fixture | Reader setup/login, own token issue, Resource `status` 200 and revoked token 401 passed with PostgreSQL and MinIO |

### Profile Evidence

| Check | Result |
|---|---|
| API | Own profile, same-account name change, CSRF/Origin, token-session denial and audit metadata passed |
| Browser | 226 Playwright tests passed, including profile edit and return to original name |
| Real HTTPS fixture | Password user changed and restored display name while retaining account identity and role |

### Machine Interface Evidence

| Check | Result |
|---|---|
| Real HTTPS fixture | Reader's own `gsm_` token passed `gscli status`, MCP `status` and Resource `status`; identity account API rejected it |
| Revocation | The same token was rejected by CLI, MCP and Resource immediately after self-revocation |
| Role freshness | MCP and Resource tests cover Reader write denial after demotion and disabled-account rejection |
| Expiry | Credential lookup excludes expired tokens; existing API lifecycle tests exercise expiry |
| Console | Issuance displays CLI and MCP connection details without putting the token into a command snippet |
