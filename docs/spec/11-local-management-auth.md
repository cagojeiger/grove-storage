# Local Management Authentication

Status: implementation in progress. Console routes accept password sessions only;
legacy Root/Master persistence and service code remain pending cleanup.

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
| Overview         | Accounts                       [+ Add account] |
| Resources        | Search [          ]  Status [Active v]         |
|   Storage        | Name       Username    Role      Status        |
|   Clients        | Heeyong    heeyong     Admin     Active        |
| Management       | Backup     backup      Writer    Pending setup |
|   Accounts       |                         [<] 1 / ... [>]         |
|   Activity       |                                               |
+------------------+-----------------------------------------------+
| Account menu --> My account: Profile | Password | Tokens | Sessions|
+------------------------------------------------------------------+
```

| Flow | UX / server result |
|---|---|
| First installation | Operator initializes an admin with hidden password input; browser uses normal Sign in |
| Add account | Admin enters username, display name and role; one-time setup link is shown once |
| First password | Recipient opens setup link, chooses password, then signs in normally |
| Expired/used setup | Generic unavailable link state; admin can issue a replacement |
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
  +-- api_tokens                 named, expiring management tokens
  +-- password_setup_tokens      expiring single-use initial setup challenges

audit_events                     committed administrative changes
security_events                  authentication and authorization events
command_invocations              management command outcomes
login_budget                     bounded shared login admission
```

Local usernames belong to password credentials, not the account identity. An
OIDC identity can later reference the same account through `(issuer, subject)`.
Username normalization is lowercase ASCII, 3-64 characters, using letters,
digits, dot, hyphen and underscore; it begins with a letter or digit.
Display names remain independent and may contain Unicode.

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
| `GET /me/sessions`, `DELETE /me/sessions/{id}` | Own sessions |
| `/accounts`, `/accounts/{id}` | Admin account lifecycle; account read includes username and password readiness |
| `POST /accounts` with `kind=user_with_password_setup` | Admin password reauthentication -> atomic account, username and single-use setup link |
| `POST /accounts/{id}/password-setup` | Admin issues/replaces initial setup challenge |
| `POST /password-setup/inspect` | Challenge in request body -> username and expiry, without consuming it |
| `POST /password-setup` | Single-use challenge + chosen password; no automatic login |
| Existing history routes | Role-scoped administrative history |

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
| 2. Session cutover | Login, expiry, CSRF, generation race, password-change and revocation tests | Verified locally; legacy fallback remains for Stage 5 |
| 3. Account UX | Setup-once flow, role/last-admin guards, personal tokens/sessions | Verified locally; legacy paths remain for Stage 5 |
| 4. Machine interfaces | CLI/MCP parity, role change, expiry/revoke, identity API denial | Verified locally |
| 5. Cleanup and release readiness | Remove old Root/Master flow, align docs, full regression and responsive browser tests | Console route/UI cutover verified; internal cleanup pending |

Existing migration checksums and Resource tables remain intact. Incremental
Management migrations support staged development; temporary legacy paths are
removed before completion. Deployment and OIDC integration are separate work.

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
issuance requires current-password verification and the shared login budget.
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
