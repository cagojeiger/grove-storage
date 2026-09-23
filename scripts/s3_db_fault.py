"""Commit failure injection for the disposable recovery-test database."""

import uuid


def install(sql, file_id):
    file_id = str(uuid.UUID(file_id))
    # Sequence increments survive rollback, proving the deferred trigger ran.
    sql(f"""
        CREATE SEQUENCE recovery_commit_failures;
        CREATE FUNCTION reject_recovery_commit() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF OLD.file_id = '{file_id}'::uuid THEN
                PERFORM nextval('recovery_commit_failures');
                RAISE EXCEPTION 'injected recovery commit failure';
            END IF;
            RETURN OLD;
        END;
        $$;
        CREATE CONSTRAINT TRIGGER recovery_commit_failure
        AFTER DELETE ON s3_uploads DEFERRABLE INITIALLY DEFERRED
        FOR EACH ROW EXECUTE FUNCTION reject_recovery_commit();
    """)


def failures(sql):
    return int(sql("SELECT CASE WHEN is_called THEN last_value ELSE 0 END FROM recovery_commit_failures"))


def remove(sql):
    sql("DROP TRIGGER recovery_commit_failure ON s3_uploads; "
        "DROP FUNCTION reject_recovery_commit(); DROP SEQUENCE recovery_commit_failures;")
