-- Root console authority is separate from existing single-use setup sessions.
CREATE TABLE management.root_sessions (
    id uuid PRIMARY KEY,
    session_hash text NOT NULL UNIQUE CHECK (session_hash ~ '^[0-9a-f]{64}$'),
    generation bigint NOT NULL CHECK (generation > 0),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    CHECK (expires_at > created_at)
);
