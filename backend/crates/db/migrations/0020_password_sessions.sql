-- Password sessions are account-bound; token sessions keep their existing
-- credential expiry and revocation behavior during the console transition.
ALTER TABLE management.sessions
    ADD COLUMN password_generation uuid;

ALTER TABLE management.sessions DROP CONSTRAINT sessions_auth_method_check;
ALTER TABLE management.sessions ADD CONSTRAINT sessions_auth_method_check
    CHECK (auth_method IN ('token', 'password', 'master'));

ALTER TABLE management.sessions DROP CONSTRAINT sessions_check1;
ALTER TABLE management.sessions ADD CONSTRAINT sessions_check1 CHECK (
    (auth_method = 'token' AND user_id IS NOT NULL AND credential_id IS NOT NULL
        AND password_generation IS NULL AND master_generation IS NULL)
    OR (auth_method = 'password' AND user_id IS NOT NULL AND credential_id IS NULL
        AND password_generation IS NOT NULL AND master_generation IS NULL)
    OR (auth_method = 'master' AND user_id IS NULL AND credential_id IS NULL
        AND password_generation IS NULL AND master_generation IS NOT NULL
        AND length(master_generation) BETWEEN 1 AND 128)
);
CREATE INDEX sessions_password_account ON management.sessions(user_id, created_at, id)
    WHERE auth_method='password' AND revoked_at IS NULL;
