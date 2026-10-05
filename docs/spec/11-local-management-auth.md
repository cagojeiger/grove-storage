# Local Management Authentication

Status: locally implemented and verified. Console routes accept password sessions
only. Migration 0024 removes inactive Root/Master authentication structures;
their original migrations and historical audit actors remain. Deployment and
OIDC are separate work.

## Boundaries

```text
Console ---- password --> session --+
CLI -------- API token --------------+--> Management --> PostgreSQL
MCP -------- API token --------------+

Client ----- service key ----------------> Native / S3 file APIs
Operator --- server-local command -------> initialize / recover account
```

| Surface | Authentication | Authority |
|---|---|---|
| Console | Username/password, then server session | Current account role |
| Console account administration | Admin session | Accounts, roles, activation, setup links |
| Console personal settings | Own session | Password, own tokens and sessions |
| Management API, CLI, MCP | Named account API token | Resource commands, bounded by current role |
| Native/S3 API | Existing Client service credentials | Existing file contract |
| Server-local operation | Server/DB operator access | Initial admin and account recovery |

Account, session and token revocation take effect on the next authorized request.
CLI/MCP share resource commands and outcomes. Identity administration belongs to
the console session boundary, including for tokens owned by an admin.

### Legacy Operator Cutover

| Setting / path | Contract |
|---|---|
| `FILEGATE_LEGACY_ADMIN_ENABLED=false` (default) | `/api/admin/v1` and all descendants return 410 `legacy_admin_disabled`, without authentication or DB access |
| Explicit `true` | Retains old `fgop_`, environment operator tokens, `fgss_` sessions and REST resource routes; startup emits a warning |
| `filegate admin init/recover/token create` | Requires explicit legacy mode |
| `filegate admin token list/revoke` | Available locally for retiring old credentials even with legacy mode disabled |
| Startup | An active local password admin, or explicitly enabled and initialized legacy authentication |
| Database | Existing operator credentials, sessions and audit records remain; no migration deletes them |

Initialize an Account with `filegate account init`, migrate management callers to
`gscli`/MCP/command API with named `gsm_` tokens, then disable legacy mode on every
API replica. Old operator authority is independent of Account roles and revocation
while compatibility mode is enabled. Turning it off is a process configuration
change, not token revocation; turning it back on restores still-valid credentials.
Native/S3 Client keys and presigned/relay contracts are unchanged. Do not deploy
the new default to a legacy-only installation before choosing its migration mode.

## Screens

```text
Sign in                         Set password (one-time setup)
+--------------------------+    +--------------------------+
| Username                 |    | Username (read-only)     |
| Password             [o] |    | New password         [o] |
|                [Sign in] |    | Confirm password     [o] |
| Recovery help            |    |           [Set password] |
+--------------------------+    +--------------------------+

Console
+------------------+-----------------------------------------------+
| Overview         | Accounts                    [+ Create account] |
| Resources        | Search [      ] Role [All v] Status [Current v] |
|   Storage        | Account                 Role     Status        |
|   Clients        | Heeyong                 Admin    Active        |
| Management       | Backup                  Writer   Pending setup |
|   Accounts       | Rows [50 v]       2 on this page    [<] [>]     |
|   Activity       |                                               |
+------------------+-----------------------------------------------+
| Sidebar account menu --> My account: Profile | Tokens | Sessions  |
|                     --> Security: Change password                |
+------------------------------------------------------------------+
```

| Flow | UX / server result |
|---|---|
| First installation | Operator initializes an admin with hidden password input; browser uses normal Sign in |
| Create account | Admin enters username, display name and role; one-time setup link is shown once |
| First password | Recipient opens setup link, confirms the read-only reserved username, chooses password, then signs in normally |
| Expired/used setup | Generic unavailable link state; replacement is available only while the account has no password |
| Password already configured | Setup-link action is disabled; use password change or operator recovery |
| Change password | Current/new/confirmation; all sessions revoked; normal sign-in follows |
| Forgotten password | Recovery help points to server operator; local recovery revokes sessions and API tokens |
| Issue token | Name and expiry; secret displayed once; UI provides CLI/MCP connection snippets |
| Revoke token | Confirmation names token and affected automation; no Client key changes |
| Role/activation | Server guard protects last usable admin; UI explains rejected operation |
| Session expired | Return to sign-in with a local return path; discard sensitive form values |

All UI labels are English. Layouts support light/dark and phone/tablet/desktop.
Tables retain cursor pagination. Loading, empty, validation, conflict, forbidden,
rate-limited and unavailable states are explicit. Forms support keyboard focus,
password-manager autocomplete and paste. Secrets remain outside persistent web
storage, URLs sent to the server, analytics and logs.

## Persistence

```text
management.accounts              identity / role / active state
  +-- password_credentials       username / Argon2id PHC / generation
  +-- sessions                   account-bound opaque browser sessions
  +-- credentials                named, expiring management API tokens
  +-- password_setup_tokens      expiring single-use initial setup challenges
  +-- authentication_budgets     separate login / reauthentication admission

audit_events                     committed administrative changes
security_events                  authentication and authorization events
command_invocations              management command outcomes
login_budget                     fixed fallback for unknown users / legacy token exchange
```

Accounts have no subtype discriminator: every account has its own role and
activation state. Sessions use a mandatory `account_id`; token sessions reference
that same account's credential through a composite foreign key, while password
sessions carry a password generation. These management identities are independent
of Client S3 credentials and provider secrets.

### Schema Cleanup / Offline Upgrade

Migration `0024_account_authentication_cleanup.sql` is transactional:

| Before | After / preservation |
|---|---|
| Account `kind='user'` | Constant column and dependent checks removed; account IDs, roles, names and state unchanged |
| Session `user_id` | Renamed to mandatory `account_id`; ownership foreign keys and expiry rules retained |
| Token/password sessions | IDs, hashes, generations, expiry and revocation timestamps unchanged |
| Master sessions, `master_configuration`, `root_sessions` | Obsolete credentials/sessions removed; never converted into Account authority |
| Historical Master/Agent audit actors | Preserved as recorded, including session IDs and owner snapshots |
| `admin_*` compatibility tables | Unchanged; explicit legacy mode remains a separate cutover decision |
| Storage/Client/file/upload tables | Unchanged; S3 protocol, SigV4 keys and object locations unchanged |

Stop **all old servers and other DB writers** before applying this migration.
Back up the DB and verify restoration first, then migrate and start the matching
new binaries. Do not run old binaries against the new schema; image-only rollback
is insufficient, so rollback requires the matching pre-upgrade database backup.
The migration lock is not a substitute for stopping old writers.

Console JSON retains the compatibility fields `kind: "user"` and `user_id`;
they no longer reflect separate DB identity types. External S3 clients require
no protocol or credential change for this cleanup.

Local usernames belong to password credentials, not the account identity. An
OIDC identity can later reference the same account through `(issuer, subject)`.
Username normalization is lowercase ASCII, 3-64 characters, using letters,
digits, dot, hyphen and underscore; it begins with a letter or digit.
Display names remain independent and may contain Unicode.

Password admission allows 60 attempts per minute per account and purpose. Login
and authenticated password checks have separate budgets; password changes, setup
issuance and token issuance share the account's reauthentication budget. Attempts
include successes. PostgreSQL atomically enforces the limit across API replicas.
Unknown users and legacy token exchanges use the existing shared 60/minute fallback;
arbitrary usernames create no budget rows. DB errors fail closed. HTTP 429 includes
`Retry-After: 60`; window expiry admits the next attempt. This is not a network
DoS defense: ingress limits and bounded hash workers remain separate protections.
Account existence is not guaranteed to be concealed by rate-limit responses.

Password credentials store a salted Argon2id PHC string and a random generation.
Passwords use NFC normalization, 15-128 Unicode characters and a local weak-value
blocklist. There are no composition or periodic-rotation requirements. The initial
Argon2id cost is 19 MiB, two iterations and one lane; hashing uses bounded blocking
workers, outside DB transactions. These choices are a product baseline, not a
claim of NIST or ISO compliance.

| Operation | Atomic DB boundary |
|---|---|
| Initialize | First account + password credential + audit; concurrent initialization has one winner |
| Login | Verify outside lock; under lock recheck credential generation/account state, then insert session |
| Change password | Recheck current session and verified generation; replace hash/generation + revoke sessions + audit |
| Recover | Replace hash/generation + revoke sessions and management tokens + invalidate setup challenges + audit |
| Complete setup | Consume current challenge + set password/generation + audit; one concurrent winner |

The role and activation guard preserves at least one active, password-ready
Admin. A pending setup does not count. Before local password initialization,
the legacy active-Admin count guard remains in force. Password browser sessions
expire after eight hours.

Password changes preserve API tokens; recovery revokes them. Neither changes
Client service keys, provider secrets or object metadata. Recovery accepts an
existing active account and preserves its ID, role and existing login name.

## API and Transport Contract

Paths below are relative to the console identity API prefix.

| Method / path | Contract |
|---|---|
| `POST /session` | Username/password -> opaque cookie; token-to-session login is rejected |
| `GET /session`, `DELETE /session` | Current identity / logout |
| `GET /me`, `PATCH /me` | Password-session-only own profile; PATCH changes display name |
| `POST /me/password` | Current/new password -> revoke sessions |
| `GET /me/tokens`, `POST /me/tokens`, `DELETE /me/tokens/{id}` | Password-session-only own named API tokens; issuance rechecks current password |
| `POST /accounts/{id}/credentials` | Admin password session and acting admin's `current_password`; issuance rechecks session and role under the identity lock after password verification |
| `GET /me/sessions`, `DELETE /me/sessions/{id}` | Own sessions |
| `/accounts`, `/accounts/{id}` | Admin account lifecycle; account read includes username and password readiness |
| `POST /accounts` with `kind=user_with_password_setup` | Admin password reauthentication -> atomic account, username and single-use setup link |
| `POST /accounts/{id}/password-setup` | Admin issues/replaces initial setup challenge |
| `POST /password-setup/inspect` | Challenge in request body -> username and expiry, without consuming it |
| `POST /password-setup` | Single-use challenge + chosen password; no automatic login |
| Existing history routes | Role-scoped administrative history |

Authenticated local account, profile and personal-token operations record
command outcomes under the response request ID. Successful mutations also
write their audit event in the same transaction as the change.

Setup links put their secret in a fragment, immediately remove it from browser
history and submit it in a body. The setup page uses no third-party content.
An initialized account uses password change/local recovery instead of initial
setup. Admin reauthentication protects account changes and sensitive issuance.

Console uses HTTPS, HttpOnly/Secure/SameSite cookies, same-origin checks and CSRF
protection. Front proxy authentication is additional admission, not a Grove
identity source. Machine API paths return JSON errors rather than browser IdP
redirects and use their own bearer-token authentication.

CLI stores the endpoint separately from credentials, accepts protected credential
storage/environment input and masks secrets. MCP authenticates the connection
with the same token contract. Passwords stay out of remote CLI/MCP configuration.
Token identity and request ID are recorded; the client-reported tool name is not
used as authorization evidence.

## Delivery Gates

| Stage | Required evidence | State |
|---|---|---|
| 1. Password foundation | Policy/hash tests, initialize/recover atomicity and concurrency tests | Verified locally |
| 2. Session cutover | Login, expiry, CSRF, generation race, password-change and revocation tests | Verified locally |
| 3. Account UX | Setup-once flow, role/last-admin guards, personal tokens/sessions | Verified locally |
| 4. Machine interfaces | CLI/MCP parity, role change, expiry/revoke, identity API denial | Verified locally |
| 5. Cleanup and release readiness | Remove old Root/Master flow, align docs, full regression and responsive browser tests | Local verification; migration 0024 implements historical authentication cleanup |

Existing migration checksums and Resource tables remain intact. Incremental
Management migrations support staged development. Migration 0024 removes the
obsolete authentication tables without rewriting earlier migration checksums.
Deployment and OIDC integration are separate work.

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

Local provisioning commands:

```sh
filegate account init owner "Owner"
filegate account recover <account-id> owner --yes
```

`FILEGATE_DATABASE_URL` selects the database. Password input is hidden and
confirmed in a terminal. The commands return account/request IDs, never the
password or PHC hash. These are server operator commands, separate from `gscli`.

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

Console creation atomically reserves the username and issues a setup link; the
link is displayed once. Account lists/details show username and password
readiness. Legacy account creation without a password remains for existing
callers during migration; it requires separate link issuance to enable password
login. A password session can issue, list and revoke its own management tokens;
issuance requires current-password verification and the account's reauthentication budget.
Legacy token sessions cannot use the personal token routes.

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

## References

- [OWASP Password Storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
- [OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [OWASP Forgot Password](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html)
- [NIST SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b.html)
