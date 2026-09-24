-- Additive foundation. Legacy admin authentication keeps its existing tables.
CREATE SCHEMA management;

CREATE TABLE management.accounts (
    id uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('user', 'agent')),
    display_name text NOT NULL CHECK (length(btrim(display_name)) BETWEEN 1 AND 80),
    role text NOT NULL CHECK (role IN ('viewer', 'operator', 'admin')),
    is_active boolean NOT NULL DEFAULT true,
    deleted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    user_id uuid GENERATED ALWAYS AS (CASE WHEN kind = 'user' THEN id END) STORED,
    agent_id uuid GENERATED ALWAYS AS (CASE WHEN kind = 'agent' THEN id END) STORED,
    UNIQUE (id, kind),
    CHECK (kind = 'user' OR role <> 'admin'),
    CHECK (deleted_at IS NULL OR NOT is_active)
);
CREATE TABLE management.users (
    account_id uuid PRIMARY KEY,
    kind text NOT NULL DEFAULT 'user' CHECK (kind = 'user'),
    FOREIGN KEY (account_id, kind) REFERENCES management.accounts(id, kind)
);
CREATE TABLE management.agents (
    account_id uuid PRIMARY KEY,
    kind text NOT NULL DEFAULT 'agent' CHECK (kind = 'agent'),
    owner_user_id uuid NOT NULL REFERENCES management.users(account_id),
    FOREIGN KEY (account_id, kind) REFERENCES management.accounts(id, kind)
);
CREATE INDEX agents_owner ON management.agents(owner_user_id);
-- Cyclic deferred FKs require exactly one matching subtype at commit.
ALTER TABLE management.accounts
    ADD FOREIGN KEY (user_id) REFERENCES management.users(account_id) DEFERRABLE INITIALLY DEFERRED,
    ADD FOREIGN KEY (agent_id) REFERENCES management.agents(account_id) DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE management.credentials (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES management.accounts(id),
    label text NOT NULL CHECK (length(btrim(label)) BETWEEN 1 AND 80),
    token_prefix text NOT NULL CHECK (token_prefix ~ '^[a-zA-Z0-9_]{1,32}$'),
    token_hash text NOT NULL UNIQUE CHECK (token_hash ~ '^[0-9a-f]{64}$'),
    hash_version smallint NOT NULL DEFAULT 1 CHECK (hash_version = 1),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    last_used_at timestamptz,
    UNIQUE (account_id, id),
    CHECK (expires_at > created_at)
);
CREATE INDEX credentials_account ON management.credentials(account_id);

CREATE TABLE management.sessions (
    id uuid PRIMARY KEY,
    session_hash text NOT NULL UNIQUE CHECK (session_hash ~ '^[0-9a-f]{64}$'),
    auth_method text NOT NULL CHECK (auth_method IN ('token', 'master')),
    user_id uuid REFERENCES management.users(account_id),
    credential_id uuid,
    master_generation text,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    FOREIGN KEY (user_id, credential_id) REFERENCES management.credentials(account_id, id),
    CHECK (expires_at > created_at),
    CHECK ((auth_method = 'token' AND user_id IS NOT NULL AND credential_id IS NOT NULL AND master_generation IS NULL)
        OR (auth_method = 'master' AND user_id IS NULL AND credential_id IS NULL
            AND master_generation IS NOT NULL AND length(master_generation) BETWEEN 1 AND 128))
);
CREATE INDEX sessions_credential ON management.sessions(credential_id);
CREATE INDEX sessions_user ON management.sessions(user_id);
CREATE INDEX sessions_expiry ON management.sessions(expires_at);

-- Actor/target IDs are snapshots, deliberately independent of account deletion.
CREATE TABLE management.audit_events (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    actor_kind text NOT NULL CHECK (actor_kind IN ('user', 'agent', 'master', 'system')),
    actor_id uuid,
    owner_user_id uuid,
    credential_id uuid,
    session_id uuid,
    request_id uuid NOT NULL,
    surface text NOT NULL CHECK (surface IN ('console', 'cli', 'mcp', 'resource_api')),
    action text NOT NULL CHECK (length(action) BETWEEN 1 AND 80),
    resource_type text NOT NULL CHECK (length(resource_type) BETWEEN 1 AND 40),
    resource_id text NOT NULL CHECK (length(resource_id) BETWEEN 1 AND 256),
    metadata jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(metadata) = 'object' AND octet_length(metadata::text) <= 8192),
    CHECK ((actor_kind IN ('user', 'agent') AND actor_id IS NOT NULL)
        OR (actor_kind IN ('master', 'system') AND actor_id IS NULL)),
    CHECK ((actor_kind = 'agent' AND owner_user_id IS NOT NULL)
        OR (actor_kind <> 'agent' AND owner_user_id IS NULL))
);
CREATE INDEX audit_time ON management.audit_events(created_at, id);
CREATE INDEX audit_actor ON management.audit_events(actor_id, created_at, id);
CREATE INDEX audit_owner ON management.audit_events(owner_user_id, created_at, id);
CREATE INDEX audit_request ON management.audit_events(request_id);
CREATE INDEX audit_target ON management.audit_events(resource_type, resource_id, created_at, id);
