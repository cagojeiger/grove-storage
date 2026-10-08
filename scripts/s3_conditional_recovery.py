"""Conditional PUT recovery after a real provider write and failed DB commit."""

import uuid

from botocore.exceptions import ClientError


def check_conditional_recovery(client, sql, backend, restart, wait_for, ready):
    for occupied in (False, True):
        key = "conditional-recovery-" + str(occupied).lower()
        args = {"Bucket": "recovery", "Key": key}
        body = b"abandoned conditional upload"
        sql(f"""
            CREATE FUNCTION reject_conditional_commit() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN
                IF OLD.protocol = 's3' AND OLD.key = '{key}' THEN
                    RAISE EXCEPTION 'injected conditional commit failure';
                END IF;
                RETURN OLD;
            END;
            $$;
            CREATE CONSTRAINT TRIGGER conditional_commit_failure
            AFTER DELETE ON uploads DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW EXECUTE FUNCTION reject_conditional_commit();
        """)
        try:
            client.put_object(**args, Body=body, IfNoneMatch="*")
        except ClientError as error:
            assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 500, error
        else:
            raise AssertionError("injected commit failure must not report success")
        file_id = str(uuid.UUID(sql(f"SELECT file_id FROM uploads WHERE protocol = 's3' AND key = '{key}'")))
        assert sql(f"SELECT state || ':' || if_none_match FROM uploads WHERE file_id = '{file_id}'") == "completing:true"
        assert sql(f"SELECT count(*) FROM s3_object_keys WHERE client_id = 'recovery' AND key = '{key}'") == "0"
        physical = sql(f"SELECT object_key FROM locations WHERE file_id = '{file_id}'")
        assert backend.vendor.get_object(Bucket=backend.spec["bucket"], Key=physical)["Body"].read() == body
        sql("DROP TRIGGER conditional_commit_failure ON uploads; DROP FUNCTION reject_conditional_commit();")
        if occupied:
            client.put_object(**args, Body=b"new winner")
        restart()
        wait_for(ready)
        assert sql(f"SELECT if_none_match FROM uploads WHERE file_id = '{file_id}'") == "t"
        sql(f"UPDATE leases SET expires_at = now() - interval '1 second' WHERE file_id = '{file_id}' AND kind = 'write'")
        expected_state = "reclaimed" if occupied else "active"
        wait_for(lambda: sql(f"SELECT state FROM files WHERE id = '{file_id}'") == expected_state)
        expected_body = b"new winner" if occupied else body
        assert client.get_object(**args)["Body"].read() == expected_body
        assert sql(f"SELECT count(*) FROM uploads WHERE file_id = '{file_id}'") == "0"
        if occupied:
            try:
                backend.vendor.head_object(Bucket=backend.spec["bucket"], Key=physical)
            except ClientError as error:
                assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 404, error
            else:
                raise AssertionError("rejected conditional bytes were not purged")
        client.delete_object(**args)
        print("PASS conditional PUT restart recovery:",
              "preserves winner and purges loser" if occupied else "publishes absent key")
