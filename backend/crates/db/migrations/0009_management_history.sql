CREATE TABLE management.command_invocations (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    actor_kind text NOT NULL CHECK (actor_kind IN ('user','agent','master')),
    actor_id uuid,
    owner_user_id uuid,
    credential_id uuid,
    session_id uuid,
    request_id uuid NOT NULL,
    surface text NOT NULL CHECK (surface IN ('console','cli','mcp','resource_api')),
    operation text NOT NULL CHECK (length(operation) BETWEEN 1 AND 80),
    outcome text NOT NULL CHECK (outcome IN ('succeeded','denied','failed','unknown')),
    error_code text CHECK (length(error_code) BETWEEN 1 AND 80),
    duration_ms bigint NOT NULL CHECK (duration_ms >= 0),
    CHECK ((actor_kind='master' AND actor_id IS NULL) OR (actor_kind IN ('user','agent') AND actor_id IS NOT NULL)),
    CHECK ((actor_kind='agent' AND owner_user_id IS NOT NULL) OR (actor_kind<>'agent' AND owner_user_id IS NULL))
);
CREATE INDEX invocation_actor ON management.command_invocations(actor_id,id DESC);
CREATE INDEX invocation_owner ON management.command_invocations(owner_user_id,id DESC);
CREATE INDEX invocation_request ON management.command_invocations(request_id);
CREATE INDEX invocation_time ON management.command_invocations(created_at,id);

CREATE TABLE management.security_events (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    actor_kind text NOT NULL CHECK (actor_kind IN ('user','agent','master','anonymous')),
    actor_id uuid,
    owner_user_id uuid,
    credential_id uuid,
    session_id uuid,
    request_id uuid NOT NULL,
    surface text NOT NULL CHECK (surface IN ('console','cli','mcp','resource_api')),
    event_type text NOT NULL CHECK (event_type IN ('authentication_failed','permission_denied','authentication_unavailable')),
    reason_code text NOT NULL CHECK (reason_code IN ('unauthenticated','forbidden','unavailable')),
    CHECK ((actor_kind IN ('master','anonymous') AND actor_id IS NULL) OR (actor_kind IN ('user','agent') AND actor_id IS NOT NULL)),
    CHECK ((actor_kind='agent' AND owner_user_id IS NOT NULL) OR (actor_kind<>'agent' AND owner_user_id IS NULL))
);
CREATE INDEX security_request ON management.security_events(request_id);
CREATE INDEX security_time ON management.security_events(created_at,id);

CREATE TABLE management.login_budget (
    id smallint PRIMARY KEY CHECK (id=1),
    window_start timestamptz NOT NULL DEFAULT clock_timestamp(),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 60)
);
INSERT INTO management.login_budget(id) VALUES(1);

CREATE INDEX audit_actor_cursor ON management.audit_events(actor_id,id DESC);
CREATE INDEX audit_owner_cursor ON management.audit_events(owner_user_id,id DESC);
