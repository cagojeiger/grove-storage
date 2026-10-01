-- Deploy with old API writers stopped. Tokens, sessions and audit snapshots
-- retain their identity; only the current role names change.
SELECT pg_advisory_xact_lock(5139268467995599950);
ALTER TABLE management.accounts DROP CONSTRAINT accounts_role_check;
UPDATE management.accounts
SET role=CASE role WHEN 'viewer' THEN 'reader' WHEN 'operator' THEN 'writer' END
WHERE role IN ('viewer','operator');
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE management.accounts ADD CONSTRAINT accounts_role_check
    CHECK (role IN ('reader','writer','admin'));
