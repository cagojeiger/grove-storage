ALTER TABLE files
    ADD COLUMN recovery_after timestamptz NOT NULL DEFAULT grove_time.transaction_now();

CREATE INDEX files_recovery ON files(state, recovery_after, id)
    WHERE state <> 'active';
