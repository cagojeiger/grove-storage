-- A pending local login reserves its unique username without a usable password.
ALTER TABLE management.password_credentials
    ALTER COLUMN password_hash DROP NOT NULL,
    ALTER COLUMN generation DROP NOT NULL,
    ALTER COLUMN password_changed_at DROP NOT NULL;
ALTER TABLE management.password_credentials
    ADD CONSTRAINT password_credentials_ready_check CHECK (
        (password_hash IS NULL AND generation IS NULL AND password_changed_at IS NULL)
        OR (password_hash IS NOT NULL AND generation IS NOT NULL AND password_changed_at IS NOT NULL)
    );

CREATE TABLE management.password_setup_tokens (
    account_id uuid PRIMARY KEY REFERENCES management.accounts(id),
    token_hash text NOT NULL UNIQUE CHECK (token_hash ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz NOT NULL CHECK (expires_at > created_at)
);
