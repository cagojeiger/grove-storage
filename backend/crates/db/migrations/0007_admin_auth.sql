CREATE TABLE admin_principals (
    id smallint PRIMARY KEY CHECK (id = 1),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE admin_credentials (
    id uuid PRIMARY KEY,
    principal_id smallint NOT NULL REFERENCES admin_principals(id),
    label text NOT NULL CHECK (length(label) BETWEEN 1 AND 80),
    token_hash text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz
);

CREATE TABLE admin_sessions (
    session_hash text PRIMARY KEY,
    credential_id uuid NOT NULL REFERENCES admin_credentials(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);
CREATE INDEX admin_sessions_credential ON admin_sessions(credential_id);

CREATE TABLE admin_login_budget (
    id smallint PRIMARY KEY CHECK (id = 1),
    window_start timestamptz NOT NULL DEFAULT now(),
    attempts integer NOT NULL DEFAULT 0
);
INSERT INTO admin_login_budget(id) VALUES (1);

CREATE TABLE admin_audit_events (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    credential_id uuid REFERENCES admin_credentials(id),
    actor text NOT NULL,
    action text NOT NULL,
    target text NOT NULL,
    status integer,
    created_at timestamptz NOT NULL DEFAULT now()
);
