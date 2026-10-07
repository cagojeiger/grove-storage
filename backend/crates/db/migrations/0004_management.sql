CREATE SCHEMA management;

CREATE TABLE management.accounts (
    id uuid PRIMARY KEY,
    display_name text NOT NULL CHECK (length(btrim(display_name)) BETWEEN 1 AND 80),
    role text NOT NULL CHECK (role IN ('reader', 'writer', 'admin')),
    is_active boolean NOT NULL DEFAULT true,
    deleted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    updated_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    CHECK (deleted_at IS NULL OR NOT is_active)
);

CREATE TABLE management.api_tokens (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES management.accounts(id),
    label text NOT NULL CHECK (length(btrim(label)) BETWEEN 1 AND 80),
    token_prefix text NOT NULL CHECK (token_prefix ~ '^[a-zA-Z0-9_]{1,32}$'),
    token_hash text NOT NULL UNIQUE CHECK (token_hash ~ '^[0-9a-f]{64}$'),
    hash_version smallint NOT NULL DEFAULT 1 CHECK (hash_version = 1),
    created_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    last_used_at timestamptz,
    UNIQUE (account_id, id),
    CHECK (expires_at > created_at)
);
CREATE INDEX api_tokens_account ON management.api_tokens(account_id);

CREATE TABLE management.password_credentials (
    account_id uuid PRIMARY KEY REFERENCES management.accounts(id),
    login_name text NOT NULL UNIQUE CHECK (login_name ~ '^[a-z0-9][a-z0-9._-]{2,63}$'),
    password_hash text CHECK (
        length(password_hash) BETWEEN 40 AND 512
        AND password_hash LIKE '$argon2id$v=19$%'
    ),
    generation uuid,
    password_changed_at timestamptz DEFAULT grove_time.wall_now(),
    CONSTRAINT password_credentials_ready_check CHECK (
        (password_hash IS NULL AND generation IS NULL AND password_changed_at IS NULL)
        OR (password_hash IS NOT NULL AND generation IS NOT NULL AND password_changed_at IS NOT NULL)
    )
);

CREATE TABLE management.password_setup_tokens (
    account_id uuid PRIMARY KEY REFERENCES management.accounts(id),
    token_hash text NOT NULL UNIQUE CHECK (token_hash ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    expires_at timestamptz NOT NULL CHECK (expires_at > created_at)
);

CREATE TABLE management.sessions (
    id uuid PRIMARY KEY,
    session_hash text NOT NULL UNIQUE CHECK (session_hash ~ '^[0-9a-f]{64}$'),
    auth_method text NOT NULL CHECK (auth_method IN ('token', 'password')),
    account_id uuid NOT NULL REFERENCES management.accounts(id),
    credential_id uuid,
    password_generation uuid,
    created_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    FOREIGN KEY (account_id, credential_id) REFERENCES management.api_tokens(account_id, id),
    CHECK (expires_at > created_at),
    CONSTRAINT sessions_authentication_check CHECK (
        (auth_method = 'token' AND credential_id IS NOT NULL AND password_generation IS NULL)
        OR (auth_method = 'password' AND credential_id IS NULL AND password_generation IS NOT NULL)
    )
);
CREATE INDEX sessions_credential ON management.sessions(credential_id);
CREATE INDEX sessions_account ON management.sessions(account_id);
CREATE INDEX sessions_expiry ON management.sessions(expires_at);
CREATE INDEX sessions_password_account ON management.sessions(account_id, created_at, id)
    WHERE auth_method = 'password' AND revoked_at IS NULL;

CREATE TABLE management.authentication_budgets (
    account_id uuid NOT NULL REFERENCES management.accounts(id) ON DELETE CASCADE,
    purpose text NOT NULL CHECK (purpose IN ('login', 'reauthentication')),
    window_start timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    attempts integer NOT NULL CHECK (attempts BETWEEN 1 AND 60),
    PRIMARY KEY (account_id, purpose)
);

CREATE TABLE management.login_budget (
    id smallint PRIMARY KEY CHECK (id = 1),
    window_start timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 60)
);
INSERT INTO management.login_budget(id) VALUES(1);

-- Audit records are independent snapshots and do not block account deletion.
CREATE TABLE management.audit_events (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    actor_kind text NOT NULL CHECK (actor_kind IN ('account', 'system')),
    actor_id uuid,
    credential_id uuid,
    session_id uuid,
    request_id uuid NOT NULL,
    surface text NOT NULL CHECK (surface IN ('console', 'cli', 'mcp', 'resource_api', 'local')),
    action text NOT NULL CHECK (length(action) BETWEEN 1 AND 80),
    resource_type text NOT NULL CHECK (length(resource_type) BETWEEN 1 AND 40),
    resource_id text NOT NULL CHECK (length(resource_id) BETWEEN 1 AND 256),
    metadata jsonb NOT NULL DEFAULT '{}' CHECK (
        jsonb_typeof(metadata) = 'object' AND octet_length(metadata::text) <= 8192
    ),
    CHECK ((actor_kind = 'account' AND actor_id IS NOT NULL)
        OR (actor_kind = 'system' AND actor_id IS NULL))
);
CREATE INDEX audit_time ON management.audit_events(created_at, id);
CREATE INDEX audit_actor ON management.audit_events(actor_id, created_at, id);
CREATE INDEX audit_request ON management.audit_events(request_id);
CREATE INDEX audit_target ON management.audit_events(resource_type, resource_id, created_at, id);
CREATE INDEX audit_actor_cursor ON management.audit_events(actor_id, id DESC);

CREATE TABLE management.command_invocations (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    actor_kind text NOT NULL CHECK (actor_kind IN ('account', 'system')),
    actor_id uuid,
    credential_id uuid,
    session_id uuid,
    request_id uuid NOT NULL,
    surface text NOT NULL CHECK (surface IN ('console', 'cli', 'mcp', 'resource_api')),
    operation text NOT NULL CHECK (length(operation) BETWEEN 1 AND 80),
    outcome text NOT NULL CHECK (outcome IN ('succeeded', 'denied', 'failed', 'unknown')),
    error_code text CHECK (length(error_code) BETWEEN 1 AND 80),
    duration_ms bigint NOT NULL CHECK (duration_ms >= 0),
    CHECK ((actor_kind = 'system' AND actor_id IS NULL)
        OR (actor_kind = 'account' AND actor_id IS NOT NULL))
);
CREATE INDEX invocation_actor ON management.command_invocations(actor_id, id DESC);
CREATE INDEX invocation_request ON management.command_invocations(request_id);
CREATE INDEX invocation_time ON management.command_invocations(created_at, id);

CREATE TABLE management.security_events (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT grove_time.wall_now(),
    actor_kind text NOT NULL CHECK (actor_kind IN ('account', 'system', 'anonymous')),
    actor_id uuid,
    credential_id uuid,
    session_id uuid,
    request_id uuid NOT NULL,
    surface text NOT NULL CHECK (surface IN ('console', 'cli', 'mcp', 'resource_api')),
    event_type text NOT NULL CHECK (event_type IN (
        'authentication_failed', 'permission_denied', 'authentication_unavailable',
        'authentication_succeeded', 'authentication_rate_limited'
    )),
    reason_code text NOT NULL CHECK (reason_code IN (
        'unauthenticated', 'forbidden', 'unavailable', 'authenticated', 'rate_limited'
    )),
    CHECK ((actor_kind IN ('system', 'anonymous') AND actor_id IS NULL)
        OR (actor_kind = 'account' AND actor_id IS NOT NULL))
);
CREATE INDEX security_request ON management.security_events(request_id);
CREATE INDEX security_time ON management.security_events(created_at, id);
