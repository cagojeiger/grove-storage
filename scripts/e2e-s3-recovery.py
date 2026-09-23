#!/usr/bin/env python3
"""Verify ambiguous multipart completion with real MinIO and PostgreSQL."""

import json
from pathlib import Path
import runpy
import time
import urllib.request
import uuid

import boto3
from botocore.config import Config
from botocore.exceptions import ClientError

from s3_backend_fixture import docker, minio_backend
from s3_fault_proxy import lose_complete_responses


HARNESS = runpy.run_path(str(Path(__file__).with_name("e2e-cli.py")))


def check(endpoint, directory, database, backend, proxy, attempts):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def admin(method, path, body=None):
        request = urllib.request.Request(endpoint + path, method=method,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Authorization": "Bearer " + HARNESS["TOKEN"],
                     "Content-Type": "application/json"})
        with opener.open(request, timeout=5) as response:
            return json.load(response)

    def sql(statement):
        return docker("exec", database, "psql", "-U", "filegate", "-d", "filegate",
                      "-v", "ON_ERROR_STOP=1", "-At", "-c", statement)

    def wait_for(predicate):
        deadline = time.monotonic() + 30
        while not predicate():
            if time.monotonic() >= deadline:
                raise AssertionError("recovery condition did not converge")
            time.sleep(0.2)

    def ready():
        try:
            return admin("GET", "/readyz")["status"] == "ready"
        except OSError:
            return False

    wait_for(ready)
    admin("POST", "/api/admin/v1/storages", {**backend.spec, "id": "recovery", "endpoint": proxy})
    admin("POST", "/api/admin/v1/clients", {"id": "recovery", "storage_id": "recovery"})
    credential = admin("POST", "/api/admin/v1/clients/recovery/s3-credentials", {})
    client = boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
        aws_access_key_id=credential["access_key_id"], aws_secret_access_key=credential["secret_key"],
        config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
                      retries={"total_max_attempts": 1}, read_timeout=60))
    args = {"Bucket": "recovery", "Key": "overwrite"}
    old = b"previous committed object"
    new = b"replacement-" + uuid.uuid4().hex.encode()
    client.put_object(**args, Body=old)
    upload = client.create_multipart_upload(**args)["UploadId"]
    file_id = str(uuid.UUID(upload))
    part = client.upload_part(**args, UploadId=upload, PartNumber=1, Body=new)
    complete = {**args, "UploadId": upload,
                "MultipartUpload": {"Parts": [{"PartNumber": 1, "ETag": part["ETag"]}]}}

    def unavailable():
        try:
            client.complete_multipart_upload(**complete)
        except ClientError as error:
            assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 503, error
        else:
            raise AssertionError("lost completion response must not report success")

    unavailable()
    assert attempts and attempts[0][0] == 200, attempts
    assert b"CompleteMultipartUploadResult" in attempts[0][1]
    assert sum(status == 200 for status, _ in attempts) == 1
    assert sql(f"SELECT state FROM s3_uploads WHERE file_id = '{file_id}'") == "completing"
    assert client.get_object(**args)["Body"].read() == old
    physical = sql(f"SELECT object_key FROM locations WHERE file_id = '{file_id}'")
    assert backend.vendor.get_object(Bucket=backend.spec["bucket"], Key=physical)["Body"].read() == new
    count = len(attempts)
    unavailable()
    assert len(attempts) == count, "client retry must not repeat vendor completion"
    print("PASS vendor committed; response lost; old object preserved; retry fenced")

    # Advance only this fixture's abandoned lease, not the product clock or state.
    sql(f"UPDATE leases SET expires_at = now() - interval '1 second' "
        f"WHERE file_id = '{file_id}' AND kind = 'write'")
    wait_for(lambda: sql(f"SELECT state FROM files WHERE id = '{file_id}'") == "active")
    assert client.get_object(**args)["Body"].read() == new
    assert sql(f"SELECT count(*) FROM s3_uploads WHERE file_id = '{file_id}'") == "0"
    assert sql(f"SELECT state FROM leases WHERE file_id = '{file_id}' AND kind = 'write'") == "committed"
    assert sql("SELECT count(*) FROM files WHERE state = 'active'") == "1"
    assert sql(f"SELECT declared_size FROM files WHERE id = '{file_id}'") == str(len(new))
    assert not backend.vendor.list_multipart_uploads(Bucket=backend.spec["bucket"]).get("Uploads")
    assert len(attempts) == count, "recovery must observe, not repeat Complete"
    print("PASS reconciler publishes vendor bytes once and commits lease; no open multipart")
    try:
        client.complete_multipart_upload(**complete)
    except ClientError as error:
        assert error.response["Error"]["Code"] == "NoSuchUpload", error
    else:
        raise AssertionError("finalized upload must no longer be open")
    wait_for(lambda: admin("GET", "/api/admin/v1/usage")[0]["purge_pending_files"] == 0)
    usage = admin("GET", "/api/admin/v1/usage")[0]
    assert usage["active_files"] == 1 and usage["active_bytes"] == len(new), usage
    assert usage["reserved_files"] == usage["reserved_bytes"] == usage["purge_pending_bytes"] == 0, usage
    assert usage["remaining_bytes"] == backend.spec["capacity_bytes"] - len(new), usage
    objects = backend.vendor.list_objects_v2(Bucket=backend.spec["bucket"])["Contents"]
    assert [obj["Key"] for obj in objects] == [physical], objects
    assert len(attempts) == count
    print("PASS old physical object purged; usage settled; finalized retry returns NoSuchUpload")


if __name__ == "__main__":
    with minio_backend() as backend:
        with lose_complete_responses(backend.endpoint) as (proxy, attempts):
            HARNESS["main"](lambda endpoint, directory, database:
                check(endpoint, directory, database, backend, proxy, attempts), with_database=True)
