ALTER TABLE files ADD CONSTRAINT files_client_id_id_key UNIQUE (client_id, id);
DROP INDEX files_client_idx;
ALTER TABLE s3_object_keys DROP CONSTRAINT s3_object_keys_file_id_fkey;
ALTER TABLE s3_object_keys ADD CONSTRAINT s3_object_keys_owner_fkey
    FOREIGN KEY (client_id, file_id) REFERENCES files(client_id, id) ON DELETE CASCADE;
CREATE UNIQUE INDEX leases_issued_write ON leases(file_id)
    WHERE kind = 'write' AND state = 'issued';

ALTER TABLE lease_history ADD COLUMN id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY;
DROP INDEX lease_history_at_idx;
CREATE INDEX lease_history_time ON lease_history(at, id);

-- Existing snapshots have no known observation time; do not invent one.
ALTER TABLE usage_snapshots ADD COLUMN observed_at timestamptz;
ALTER TABLE usage_snapshots ALTER COLUMN observed_at SET DEFAULT grove_time.transaction_now();

CREATE INDEX sessions_retention ON management.sessions
    (LEAST(expires_at, COALESCE(revoked_at, expires_at)), id);
CREATE INDEX api_tokens_retention ON management.api_tokens
    (LEAST(expires_at, COALESCE(revoked_at, expires_at)), id);
CREATE INDEX password_setup_retention ON management.password_setup_tokens(expires_at, account_id);

-- Retain non-secret token identity before verification rows can be pruned.
UPDATE management.audit_events e SET metadata = e.metadata || jsonb_build_object(
    'account_id', t.account_id, 'label', t.label,
    'expires_at', t.expires_at)
FROM management.api_tokens t
WHERE e.resource_type = 'credential' AND e.resource_id = t.id::text
    AND e.action IN ('credential.issue', 'credential.revoke');
