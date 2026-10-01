-- Stop old writers before applying this migration. Preserve account IDs and
-- historical actor snapshots; old Agent credentials never gain User access.
SELECT pg_advisory_xact_lock(5139268467995599950);

UPDATE management.credentials SET revoked_at=COALESCE(revoked_at,clock_timestamp())
WHERE account_id IN (SELECT id FROM management.accounts WHERE kind='agent');
UPDATE management.sessions SET revoked_at=COALESCE(revoked_at,clock_timestamp())
WHERE user_id IN (SELECT id FROM management.accounts WHERE kind='agent');

ALTER TABLE management.accounts DROP COLUMN agent_id;
DROP TABLE management.agents;
UPDATE management.accounts SET kind='user',is_active=false,updated_at=clock_timestamp()
WHERE kind='agent';
INSERT INTO management.users(account_id)
SELECT id FROM management.accounts
WHERE NOT EXISTS (SELECT 1 FROM management.users WHERE account_id=id);
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE management.accounts ADD CONSTRAINT accounts_user_only CHECK (kind='user');
