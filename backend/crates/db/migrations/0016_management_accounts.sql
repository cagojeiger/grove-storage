-- Stop old writers before applying: accounts is the sole current identity table.
-- The migration runner applies these changes in one transaction.
SELECT pg_advisory_xact_lock(5139268467995599950);

ALTER TABLE management.sessions
    DROP CONSTRAINT sessions_user_id_fkey,
    ADD CONSTRAINT sessions_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES management.accounts(id);

ALTER TABLE management.accounts DROP COLUMN user_id;
DROP TABLE management.users;
ALTER TABLE management.accounts DROP CONSTRAINT accounts_id_kind_key;
