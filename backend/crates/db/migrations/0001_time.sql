CREATE SCHEMA grove_time;

CREATE FUNCTION grove_time.transaction_now() RETURNS timestamptz
LANGUAGE sql STABLE AS $$ SELECT CURRENT_TIMESTAMP $$;

CREATE FUNCTION grove_time.wall_now() RETURNS timestamptz
LANGUAGE sql VOLATILE AS $$ SELECT clock_timestamp() $$;
