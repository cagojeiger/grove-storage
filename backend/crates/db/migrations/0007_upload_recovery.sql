ALTER TABLE uploads
    ADD COLUMN recovery_after timestamptz NOT NULL DEFAULT grove_time.transaction_now();

DROP INDEX uploads_recovery;
CREATE INDEX uploads_recovery ON uploads(protocol, state, recovery_after, file_id);
