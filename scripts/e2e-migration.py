#!/usr/bin/env python3
"""Offline FileGate v0.4.1 -> Grove -> backup restore, using disposable services only."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request

import boto3
from botocore.config import Config

from migration_fixture import CONSOLE_ORIGIN, ROOT, SERVER, rehearsal
from s3_backend_fixture import minio_backend

LEGACY_REVISION = "af685807e5561885616e00918f0207e25188fb3a"
NATIVE_KEY = "migration-fixture-native-key"
NATIVE_BYTES = b"native object created before migration"
S3_BYTES = b"S3 object created before migration"
PART = b"pending multipart before migration"


def legacy_binary(directory):
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=directory, text=True).strip()
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=directory, text=True)
    if revision != LEGACY_REVISION or dirty:
        raise RuntimeError("use a clean checkout of pinned FileGate v0.4.1")
    binary = directory / "target/debug/filegate"
    if not binary.is_file() or not SERVER.is_file():
        raise RuntimeError("build both FileGate and Grove binaries before the rehearsal")
    print("FileGate revision:", revision, "binary SHA-256:", hashlib.sha256(binary.read_bytes()).hexdigest())
    return binary


def s3(endpoint, credential):
    return boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
        aws_access_key_id=credential["access_key_id"], aws_secret_access_key=credential["secret_key"],
        config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
                      connect_timeout=3, read_timeout=10, retries={"total_max_attempts": 1}))


def put(fixture, url, body):
    with fixture.opener.open(urllib.request.Request(url, data=body, method="PUT"), timeout=10) as response:
        assert response.status == 200


def seed(fixture, backend):
    fixture.request("POST", "/api/admin/v1/storages", {"id": "archive", **backend.spec})
    fixture.request("POST", "/api/admin/v1/clients", {"id": "consumer", "storage_id": "archive"})
    fixture.request("POST", "/api/admin/v1/clients/consumer/keys",
                    {"key_hash": "sha256:" + hashlib.sha256(NATIVE_KEY.encode()).hexdigest()})
    credential = fixture.request("POST", "/api/admin/v1/clients/consumer/s3-credentials", {})
    native = fixture.request("POST", "/api/v1/files", {"declared_size": len(NATIVE_BYTES)}, NATIVE_KEY)
    put(fixture, native["put_url"], NATIVE_BYTES)
    fixture.request("POST", f"/api/v1/files/{native['file_id']}/commit", token=NATIVE_KEY)
    pending = fixture.request("POST", "/api/v1/files", {"declared_size": len(PART)}, NATIVE_KEY)
    client = s3(fixture.endpoint, credential)
    client.put_object(Bucket="consumer", Key="kept/object", Body=S3_BYTES)
    upload = client.create_multipart_upload(Bucket="consumer", Key="pending/object")["UploadId"]
    etag = client.upload_part(Bucket="consumer", Key="pending/object", UploadId=upload,
                              PartNumber=1, Body=PART)["ETag"]
    old_url = client.generate_presigned_url("get_object", Params={"Bucket": "consumer", "Key": "kept/object"})
    return {"native": native["file_id"], "pending": pending, "credential": credential,
            "upload": upload, "etag": etag, "old_url": old_url}


def vendor_snapshot(backend):
    client, bucket = backend.vendor, backend.spec["bucket"]
    objects = {}
    for page in client.get_paginator("list_objects_v2").paginate(Bucket=bucket):
        for item in page.get("Contents", []):
            response = client.get_object(Bucket=bucket, Key=item["Key"])
            with response["Body"] as body:
                objects[item["Key"]] = hashlib.sha256(body.read()).hexdigest()
    uploads = {}
    for page in client.get_paginator("list_multipart_uploads").paginate(Bucket=bucket):
        for item in page.get("Uploads", []):
            parts = []
            for listed in client.get_paginator("list_parts").paginate(
                    Bucket=bucket, Key=item["Key"], UploadId=item["UploadId"]):
                parts.extend((p["PartNumber"], p["ETag"], p["Size"]) for p in listed.get("Parts", []))
            uploads[(item["Key"], item["UploadId"])] = parts
    return objects, uploads


def verify_reads(fixture, data):
    assert fixture.request("GET", "/api/admin/v1/clients/consumer")["storage_id"] == "archive"
    path = f"/api/v1/files/{data['native']}"
    assert fixture.request("GET", path, token=NATIVE_KEY)["state"] == "active"
    read = fixture.request("POST", path + "/read", {}, NATIVE_KEY)
    with fixture.opener.open(read["get_url"], timeout=10) as response:
        assert response.read() == NATIVE_BYTES
    client = s3(fixture.endpoint, data["credential"])
    assert client.head_object(Bucket="consumer", Key="kept/object")["ContentLength"] == len(S3_BYTES)
    response = client.get_object(Bucket="consumer", Key="kept/object")
    with response["Body"] as body:
        assert body.read() == S3_BYTES
    with fixture.opener.open(data["old_url"], timeout=10) as response:
        assert response.read() == S3_BYTES
    assert fixture.request("GET", f"/api/v1/files/{data['pending']['file_id']}", token=NATIVE_KEY)["state"] == "pending"


def initialize(fixture):
    password = "a private offline migration rehearsal passphrase"
    result = subprocess.check_output(
        ["python3", "scripts/e2e-password-account.py", fixture.container], cwd=ROOT,
        env=dict(fixture.environment(), GROVE_E2E_PASSWORD=password), text=True, timeout=45)
    return json.loads(result)["account_id"], password


def verify_login(fixture, account, password):
    # Exercises the API directly; browser HTTPS/cookie enforcement has its own E2E suite.
    request = urllib.request.Request(fixture.endpoint + "/api/admin/identity/v1/session",
        data=json.dumps({"username": "owner", "password": password}).encode(),
        headers={"Content-Type": "application/json", "Origin": CONSOLE_ORIGIN, "X-Grove-CSRF": "1"})
    with fixture.opener.open(request, timeout=10) as response:
        assert json.load(response)["user_id"] == account
        cookie = response.headers["Set-Cookie"].split(";", 1)[0]
    request = urllib.request.Request(fixture.endpoint + "/api/admin/identity/v1/session",
                                     headers={"Cookie": cookie, "Origin": CONSOLE_ORIGIN})
    with fixture.opener.open(request, timeout=10) as response:
        assert json.load(response)["role"] == "admin"


def preflight(fixture, directory):
    rows = json.loads(fixture.sql("SELECT json_agg(t ORDER BY version) FROM "
        "(SELECT version,success,encode(checksum,'hex') AS checksum FROM _sqlx_migrations) t;"))
    assert [row["version"] for row in rows] == list(range(1, 7)), "expected released schema 0001..0006"
    for row in rows:
        source = next((directory / "backend/crates/db/migrations").glob(f"{row['version']:04d}_*.sql"))
        assert row["success"] and row["checksum"] == hashlib.sha384(source.read_bytes()).hexdigest()
    assert fixture.sql("SELECT count(*) FROM storages WHERE kind <> 's3';") == "0", "S3-only upgrade required"
    assert fixture.sql("SELECT count(*) FROM s3_uploads WHERE state <> 'open';") == "0", "completion recovery must finish first"
    assert fixture.sql("SELECT count(*) FROM native_multipart_completions;") == "0", "native completion recovery must finish first"


def run(directory):
    legacy = legacy_binary(directory)
    with minio_backend() as backend, rehearsal() as fixture:
        with fixture.server(legacy):
            data = seed(fixture, backend)
            verify_reads(fixture, data)
        preflight(fixture, directory)
        tables = json.loads(fixture.sql("SELECT json_agg(tablename ORDER BY tablename) "
                                       "FROM pg_tables WHERE schemaname='public';"))
        baseline = fixture.snapshot(tables)
        resources = [table for table in tables if table != "_sqlx_migrations"]
        expected = fixture.snapshot(resources, normalized=True)
        physical = vendor_snapshot(backend)
        backup = fixture.backup()
        print("PASS released FileGate schema/checksums, active objects, pending uploads and stopped-writer backup")

        account, password = initialize(fixture)
        assert fixture.snapshot(resources, normalized=True) == expected, "upgrade changed legacy resource rows"
        assert fixture.sql("SELECT count(*) FROM storages WHERE metadata <> '{}'::jsonb;") == "0"
        assert fixture.sql("SELECT count(*) FROM clients WHERE metadata <> '{}'::jsonb;") == "0"
        assert fixture.sql("SELECT count(*) FROM s3_uploads WHERE if_none_match;") == "0"
        with fixture.server(SERVER):
            verify_reads(fixture, data)
            verify_login(fixture, account, password)
        assert vendor_snapshot(backend) == physical, "S3 changed after backup; DB-only rollback is unsafe"
        print("PASS Grove upgrade preserves encrypted credentials, file IDs/locations, leases and upload IDs; Admin login works")

        incompatible = subprocess.run([str(legacy)], cwd=fixture.directory,
            env=fixture.environment(), capture_output=True, text=True, timeout=30)
        message = (incompatible.stdout + incompatible.stderr).lower()
        assert incompatible.returncode != 0 and "migration" in message and "missing" in message, \
            "old binary must reject the upgraded schema; image-only rollback is not supported"
        fixture.restore(backup)
        assert fixture.snapshot(tables, database="rollback") == baseline, "restored rows or migration checksums differ"
        assert fixture.sql("SELECT count(*) FROM pg_namespace WHERE nspname='management';", "rollback") == "0"
        with fixture.server(legacy, database="rollback"):
            verify_reads(fixture, data)
            # Resume mutations only AFTER restoring and checking the quiescent snapshot.
            put(fixture, data["pending"]["put_url"], PART)
            fixture.request("POST", f"/api/v1/files/{data['pending']['file_id']}/commit", token=NATIVE_KEY)
            read = fixture.request("POST", f"/api/v1/files/{data['pending']['file_id']}/read", {}, NATIVE_KEY)
            with fixture.opener.open(read["get_url"], timeout=10) as response:
                assert response.read() == PART
            client = s3(fixture.endpoint, data["credential"])
            client.complete_multipart_upload(Bucket="consumer", Key="pending/object", UploadId=data["upload"],
                MultipartUpload={"Parts": [{"PartNumber": 1, "ETag": data["etag"]}]})
            response = client.get_object(Bucket="consumer", Key="pending/object")
            with response["Body"] as body:
                assert body.read() == PART
            assert not backend.vendor.list_multipart_uploads(Bucket=backend.spec["bucket"]).get("Uploads")
        print("PASS image-only rollback rejected; backup restored exact legacy rows/checksums; old server reads and resumes uploads")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--filegate-dir", required=True, type=Path)
    args = parser.parse_args()
    run(args.filegate_dir.resolve())
