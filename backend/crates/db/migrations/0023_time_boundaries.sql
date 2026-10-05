-- Preserve PostgreSQL's transaction/wall-time distinction. Tests may replace
-- these dependencies in their disposable database; production has no time override.
CREATE SCHEMA grove_time;
CREATE FUNCTION grove_time.transaction_now() RETURNS timestamptz
LANGUAGE sql STABLE AS $$ SELECT CURRENT_TIMESTAMP $$;
CREATE FUNCTION grove_time.wall_now() RETURNS timestamptz
LANGUAGE sql VOLATILE AS $$ SELECT clock_timestamp() $$;

-- Route timestamp defaults through the same clock as explicit lifecycle queries.
DO $$
DECLARE column_default record;
BEGIN
    FOR column_default IN
        SELECT n.nspname, c.relname, a.attname, pg_get_expr(d.adbin, d.adrelid) AS expression
        FROM pg_attrdef d
        JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
        JOIN pg_class c ON c.oid=d.adrelid
        JOIN pg_namespace n ON n.oid=c.relnamespace
        WHERE (n.nspname='management' OR (n.nspname='public' AND c.relname IN (
            'storages','clients','client_keys','s3_credentials','s3_keys','s3_uploads',
            'files','locations','leases','lease_parts','lease_history','usage_snapshot',
            'native_multipart_completions','admin_principals','admin_credentials',
            'admin_sessions','admin_login_budget','admin_audit_events')))
          AND pg_get_expr(d.adbin, d.adrelid) IN ('now()', 'CURRENT_TIMESTAMP', 'clock_timestamp()')
    LOOP
        EXECUTE format('ALTER TABLE %I.%I ALTER COLUMN %I SET DEFAULT grove_time.%s()',
            column_default.nspname, column_default.relname, column_default.attname,
            CASE WHEN column_default.expression='clock_timestamp()' THEN 'wall_now' ELSE 'transaction_now' END);
    END LOOP;
END $$;
