-- Apply offline with all old writers stopped. Historical audit actor snapshots
-- remain unchanged; obsolete Master/Root sessions cannot authenticate today.
SELECT pg_advisory_xact_lock(5139268467995599950);
LOCK TABLE management.accounts, management.sessions IN ACCESS EXCLUSIVE MODE;

DELETE FROM management.sessions WHERE auth_method = 'master';
DROP TABLE management.root_sessions;
DROP TABLE management.master_configuration;

ALTER TABLE management.accounts DROP COLUMN kind;
ALTER TABLE management.sessions DROP CONSTRAINT sessions_check1;
ALTER TABLE management.sessions DROP CONSTRAINT sessions_auth_method_check;
DROP INDEX management.sessions_master;
ALTER TABLE management.sessions DROP COLUMN master_generation;
ALTER TABLE management.sessions RENAME COLUMN user_id TO account_id;
ALTER TABLE management.sessions ALTER COLUMN account_id SET NOT NULL;
ALTER TABLE management.sessions RENAME CONSTRAINT sessions_user_id_fkey TO sessions_account_id_fkey;
ALTER TABLE management.sessions RENAME CONSTRAINT sessions_user_id_credential_id_fkey TO sessions_account_credential_fkey;
ALTER INDEX management.sessions_user RENAME TO sessions_account;

ALTER TABLE management.sessions ADD CONSTRAINT sessions_auth_method_check
    CHECK (auth_method IN ('token', 'password'));
ALTER TABLE management.sessions ADD CONSTRAINT sessions_authentication_check CHECK (
    (auth_method = 'token' AND credential_id IS NOT NULL AND password_generation IS NULL)
    OR (auth_method = 'password' AND credential_id IS NULL AND password_generation IS NOT NULL)
);
