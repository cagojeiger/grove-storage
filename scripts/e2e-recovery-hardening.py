#!/usr/bin/env python3
"""Verify recovery fairness, committed-but-unacknowledged results, and in-flight process death."""

import hashlib
import json
from pathlib import Path
import runpy
import threading
import time
import uuid

import boto3
from botocore.config import Config
from botocore.exceptions import ClientError

from cli_management_fixture import Management
from pg_commit_proxy import CommitProxy
from s3_backend_fixture import docker, minio_backend
from s3_fault_proxy import complete_proxy

HARNESS = runpy.run_path(str(Path(__file__).with_name("e2e-cli.py")))


def sql_for(database):
    return lambda statement: docker("exec", database, "psql", "-U", "grove", "-d", "grove",
        "-v", "ON_ERROR_STOP=1", "-At", "-c", statement)


def wait_for(predicate, timeout=60):
    deadline = time.monotonic() + timeout
    while not predicate():
        if time.monotonic() >= deadline:
            raise AssertionError("recovery did not converge")
        time.sleep(0.2)


def s3_client(endpoint, management, name, backend, proxy=None):
    management.command("storage.create", {"id": name, "spec": {
        **backend.spec, "endpoint": proxy or backend.endpoint}})
    management.command("client.create", {"id": name, "storage_id": name})
    credential = management.command("credential.create", {"client_id": name})
    return boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
        aws_access_key_id=credential["access_key_id"], aws_secret_access_key=credential["secret_key"],
        config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
            retries={"total_max_attempts": 1}, connect_timeout=3, read_timeout=40))


def prepared(client, bucket):
    args = {"Bucket": bucket, "Key": "overwrite"}
    old, new = b"committed-before-fault", b"replacement-after-fault"
    client.put_object(**args, Body=old)
    upload = client.create_multipart_upload(**args)["UploadId"]
    part = client.upload_part(**args, UploadId=upload, PartNumber=1, Body=new)
    complete = {**args, "UploadId": upload,
        "MultipartUpload": {"Parts": [{"PartNumber": 1, "ETag": part["ETag"]}]}}
    return args, old, new, uuid.UUID(upload), complete


def commit_loss(endpoint, _directory, database, account, restart, backend, proxy):
    sql = sql_for(database)
    client = s3_client(endpoint, Management(endpoint, account), "ack-loss", backend)
    args, _old, new, file_id, complete = prepared(client, "ack-loss")
    proxy.target = file_id
    try:
        client.complete_multipart_upload(**complete)
    except ClientError as error:
        assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 500, error
        assert error.response["Error"]["Code"] == "InternalError", error
    else:
        raise AssertionError("COMMIT acknowledgment loss did not reach the caller")
    assert proxy.hits == 1, "PostgreSQL's successful COMMIT was not intercepted"
    assert sql(f"SELECT state FROM files WHERE id='{file_id}'") == "active"
    assert sql(f"SELECT state FROM leases WHERE file_id='{file_id}' AND kind='write'") == "committed"
    assert sql(f"SELECT count(*) FROM uploads WHERE file_id='{file_id}'") == "0"
    assert client.get_object(**args)["Body"].read() == new
    restart()
    HARNESS["verify_modern_startup"](endpoint)
    assert client.get_object(**args)["Body"].read() == new
    try:
        client.complete_multipart_upload(**complete)
    except ClientError as error:
        assert error.response["Error"]["Code"] == "NoSuchUpload", error
    else:
        raise AssertionError("finalized upload retry was accepted")
    print("PASS COMMIT acknowledgment loss: HTTP 500; durable active/committed result survives restart")


def inflight(endpoint, _directory, database, account, restart, backend, proxy, pause, attempts):
    entered, release = pause
    sql = sql_for(database)
    client = s3_client(endpoint, Management(endpoint, account), "inflight", backend, proxy)
    args, old, new, file_id, complete = prepared(client, "inflight")
    errors = []

    def complete_call():
        try:
            client.complete_multipart_upload(**complete)
        except Exception as error:
            errors.append(type(error).__name__)

    caller = threading.Thread(target=complete_call, daemon=True)
    caller.start()
    try:
        assert entered.wait(30), "provider completion was not observed"
        assert [status for status, _ in attempts] == [200]
        assert sql(f"SELECT state FROM uploads WHERE file_id='{file_id}'") == "completing"
        assert sql(f"SELECT state FROM files WHERE id='{file_id}'") == "pending"
        restart()
    finally:
        release.set()
        caller.join(timeout=10)
    assert not caller.is_alive() and errors, "in-flight request did not fail after SIGKILL"
    HARNESS["verify_modern_startup"](endpoint)
    assert client.get_object(**args)["Body"].read() == old
    # Only the abandoned fixture lease is aged; the product clock remains real.
    sql(f"UPDATE leases SET expires_at=now()-interval '1 second' WHERE file_id='{file_id}' AND kind='write'")
    wait_for(lambda: sql(f"SELECT state FROM files WHERE id='{file_id}'") == "active")
    assert client.get_object(**args)["Body"].read() == new
    assert sql(f"SELECT count(*) FROM uploads WHERE file_id='{file_id}'") == "0"
    assert [status for status, _ in attempts] == [200], "recovery repeated provider Complete"
    print("PASS SIGKILL while provider reply is pending: old read preserved; durable intent recovers new bytes")


def fairness(endpoint, _directory, database, account, backend):
    sql = sql_for(database)
    management = Management(endpoint, account)
    client = s3_client(endpoint, management, "healthy", backend)
    management.command("storage.create", {"id": "poison", "spec": backend.spec})
    management.command("client.create", {"id": "poison", "storage_id": "poison"})
    payload = b"valid-fairness-object"
    etag = hashlib.md5(payload).hexdigest()
    ids = sorted(uuid.uuid4() for _ in range(41))
    healthy = ids[-1]
    values = ",".join(f"('{item}'::uuid)" for item in ids)
    sql(f"WITH seeded AS (INSERT INTO files(id,client_id,declared_size) SELECT id,'poison',{len(payload)} "
        f"FROM (VALUES {values}) AS v(id) RETURNING id), placed AS (INSERT INTO locations(file_id,storage_id,object_key) "
        "SELECT id,'poison','fairness/'||id::text FROM seeded RETURNING file_id), leased AS ("
        "INSERT INTO leases(file_id,kind,expires_at) SELECT file_id,'write',now()+interval '1 hour' FROM placed RETURNING file_id) "
        "INSERT INTO uploads(file_id,protocol,key,multipart,state,expected_size,expected_etag) "
        f"SELECT file_id,'s3','fairness/'||file_id::text,false,'completing',{len(payload)},'{etag}' FROM leased")
    backend.vendor.put_object(Bucket=backend.spec["bucket"], Key="fairness/"+str(healthy), Body=payload)
    sql(f"UPDATE files SET client_id='healthy' WHERE id='{healthy}'; "
        f"UPDATE locations SET storage_id='healthy' WHERE file_id='{healthy}'; "
        "UPDATE storages SET access_key='revoked-fixture-access-key' WHERE id='poison'; "
        "UPDATE leases SET expires_at=now()-interval '1 hour' WHERE kind='write'")
    wait_for(lambda: sql(f"SELECT state FROM files WHERE id='{healthy}'") == "active")
    assert client.get_object(Bucket="healthy", Key="fairness/"+str(healthy))["Body"].read() == payload
    assert sql("SELECT count(*) FROM uploads WHERE state='completing'") == "40"
    assert sql("SELECT count(*) FROM uploads WHERE recovery_after>now()") != "0"
    assert sql("SELECT count(*) FROM leases le JOIN uploads u ON u.file_id=le.file_id "
        "WHERE le.state='issued' AND le.expires_at<now()") == "40"
    print("PASS 40 failed candidates retain recovery material without starving a healthy completion")


def verify_failures(log):
    failures = sum(json.loads(line).get("fields", {}).get("event") == "reconciler.observe_failed"
        for line in log.splitlines() if line.startswith(b"{"))
    assert failures >= 40, f"expected two real failed batches, saw {failures}"
    print(f"PASS real worker observed {failures} provider failures while healthy storage progressed")


if __name__ == "__main__":
    with minio_backend() as backend:
        with CommitProxy() as proxy:
            HARNESS["main"](lambda *args: commit_loss(*args, backend, proxy),
                with_database=True, with_restart=True, database_url_transform=proxy.route)
        pause = (threading.Event(), threading.Event())
        with complete_proxy(backend.endpoint, drop_response=False, pause=pause) as (proxy, attempts):
            HARNESS["main"](lambda *args: inflight(*args, backend, proxy, pause, attempts),
                with_database=True, with_restart=True)
        HARNESS["main"](lambda *args: fairness(*args, backend),
            with_database=True, verify_log=verify_failures)
