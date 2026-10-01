-- Local login identity is separate from account authorization and resource data.
CREATE TABLE management.password_credentials (
    account_id uuid PRIMARY KEY REFERENCES management.accounts(id),
    login_name text NOT NULL UNIQUE CHECK (
        login_name ~ '^[a-z0-9][a-z0-9._-]{2,63}$'
    ),
    password_hash text NOT NULL CHECK (
        length(password_hash) BETWEEN 40 AND 512
        AND password_hash LIKE '$argon2id$v=19$%'
    ),
    generation uuid NOT NULL,
    password_changed_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

ALTER TABLE management.audit_events DROP CONSTRAINT audit_events_surface_check;
ALTER TABLE management.audit_events ADD CONSTRAINT audit_events_surface_check
    CHECK (surface IN ('console', 'cli', 'mcp', 'resource_api', 'local'));
