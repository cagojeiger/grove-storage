#!/usr/bin/env python3
"""Verify a fresh Grove database, S3 contracts, restart and backup restoration."""

import hashlib
import json
import subprocess
import sys
import urllib.request

import boto3
from botocore.config import Config

from cli_management_fixture import Management
from installation_fixture import CONSOLE_ORIGIN, ROOT, SERVER, installation
from s3_backend_fixture import minio_backend

NATIVE_KEY = "installation-fixture-native-key"
NATIVE_BYTES = b"native object in a fresh Grove installation"
S3_BYTES = b"S3 object in a fresh Grove installation"
PART = b"pending multipart survives backup restoration"


def s3(endpoint, credential):
    return boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
        aws_access_key_id=credential["access_key_id"], aws_secret_access_key=credential["secret_key"],
        config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
                      connect_timeout=3, read_timeout=10, retries={"total_max_attempts": 1}))


def put(fixture, url, body):
    with fixture.opener.open(urllib.request.Request(url, data=body, method="PUT"), timeout=10) as response:
        assert response.status == 200


def seed(fixture, management, backend):
    management.command("storage.create", {"id": "archive", "spec": backend.spec})
    management.command("client.create", {"id": "consumer", "storage_id": "archive"})
    management.command("client-key.register", {
        "client_id": "consumer", "key_hash": "sha256:" + hashlib.sha256(NATIVE_KEY.encode()).hexdigest(),
    })
    credential = management.command("credential.create", {"client_id": "consumer"})
    native = fixture.request("POST", "/api/v1/files", {"declared_size": len(NATIVE_BYTES)}, NATIVE_KEY)
    put(fixture, native["put_url"], NATIVE_BYTES)
    fixture.request("POST", f"/api/v1/files/{native['file_id']}/commit", token=NATIVE_KEY)
    pending = fixture.request("POST", "/api/v1/files", {"declared_size": len(PART)}, NATIVE_KEY)
    client = s3(fixture.endpoint, credential)
    put_url = client.generate_presigned_url("put_object", Params={"Bucket": "consumer", "Key": "kept/object"})
    put(fixture, put_url, S3_BYTES)
    upload = client.create_multipart_upload(Bucket="consumer", Key="pending/object")["UploadId"]
    etag = client.upload_part(Bucket="consumer", Key="pending/object", UploadId=upload,
                              PartNumber=1, Body=PART)["ETag"]
    get_url = client.generate_presigned_url("get_object", Params={"Bucket": "consumer", "Key": "kept/object"})
    return {"native": native["file_id"], "pending": pending, "credential": credential,
            "upload": upload, "etag": etag, "get_url": get_url}


def verify_reads(fixture, data, *, pending_state="pending"):
    path = f"/api/v1/files/{data['native']}"
    assert fixture.request("GET", path, token=NATIVE_KEY)["state"] == "active"
    read = fixture.request("POST", path + "/read", {}, NATIVE_KEY)
    with fixture.opener.open(read["get_url"], timeout=10) as response:
        assert response.read() == NATIVE_BYTES
    client = s3(fixture.endpoint, data["credential"])
    assert client.head_object(Bucket="consumer", Key="kept/object")["ContentLength"] == len(S3_BYTES)
    with client.get_object(Bucket="consumer", Key="kept/object")["Body"] as body:
        assert body.read() == S3_BYTES
    with fixture.opener.open(data["get_url"], timeout=10) as response:
        assert response.read() == S3_BYTES
    assert fixture.request("GET", f"/api/v1/files/{data['pending']['file_id']}", token=NATIVE_KEY)["state"] == pending_state


def verify_schema(fixture, database="grove"):
    expected = [{
        "version": int(source.name.split("_", 1)[0]), "success": True,
        "checksum": hashlib.sha384(source.read_bytes()).hexdigest(),
    } for source in sorted((ROOT / "backend/crates/db/migrations").glob("*.sql"))]
    actual = json.loads(fixture.sql("SELECT json_agg(t ORDER BY version) FROM "
        "(SELECT version,success,encode(checksum,'hex') AS checksum FROM _sqlx_migrations) t;", database))
    assert actual == expected, "database/binary baseline does not match current SQL sources"
    assert fixture.sql("SELECT count(*) FROM pg_tables WHERE schemaname='management' "
        "AND tablename IN ('master_configuration','root_sessions','users','agents','credentials');", database) == "0"
    assert fixture.sql("SELECT count(*) FROM information_schema.columns WHERE "
        "(table_schema='public' AND table_name='storages' AND column_name='root_path') OR "
        "(table_schema='management' AND ((table_name='accounts' AND column_name='kind') OR "
        "(table_name='sessions' AND column_name IN ('user_id','master_generation'))));", database) == "0"
    assert fixture.sql("SELECT is_nullable FROM information_schema.columns WHERE table_schema='management' "
        "AND table_name='sessions' AND column_name='account_id';", database) == "NO"
    print(f"PASS fresh schema and all {len(expected)} baseline checksums match current sources")


def verify_login(fixture, owner):
    request = urllib.request.Request(fixture.endpoint + "/api/admin/identity/v1/session",
        data=json.dumps({"username": "owner", "password": owner["password"]}).encode(),
        headers={"Content-Type": "application/json", "Origin": CONSOLE_ORIGIN, "X-Grove-CSRF": "1"})
    with fixture.opener.open(request, timeout=10) as response:
        assert json.load(response)["user_id"] == owner["account_id"]
        cookie = response.headers["Set-Cookie"].split(";", 1)[0]
    request = urllib.request.Request(fixture.endpoint + "/api/admin/identity/v1/session",
                                     headers={"Cookie": cookie, "Origin": CONSOLE_ORIGIN})
    with fixture.opener.open(request, timeout=10) as response:
        assert json.load(response)["role"] == "admin"


def resume_pending(fixture, data):
    put(fixture, data["pending"]["put_url"], PART)
    fixture.request("POST", f"/api/v1/files/{data['pending']['file_id']}/commit", token=NATIVE_KEY)
    client = s3(fixture.endpoint, data["credential"])
    client.complete_multipart_upload(Bucket="consumer", Key="pending/object", UploadId=data["upload"],
        MultipartUpload={"Parts": [{"PartNumber": 1, "ETag": data["etag"]}]})
    with client.get_object(Bucket="consumer", Key="pending/object")["Body"] as body:
        assert body.read() == PART


def run():
    if not SERVER.is_file():
        raise RuntimeError("build grove-storage before installation verification")
    with minio_backend() as backend, installation() as fixture:
        password = "a private fresh installation fixture passphrase"
        owner = json.loads(subprocess.check_output(
            [sys.executable, "scripts/e2e-password-account.py", fixture.container], cwd=ROOT,
            env=dict(fixture.environment(), GROVE_E2E_PASSWORD=password), text=True, timeout=45))
        owner.update(username="owner", password=password)
        verify_schema(fixture)
        with fixture.server(SERVER):
            management = Management(fixture.endpoint, owner)
            data = seed(fixture, management, backend)
            verify_reads(fixture, data)
        print("PASS fresh initialization, password authentication, resource commands, Native and S3 presigned PUT/GET")

        tables = json.loads(fixture.sql("SELECT json_agg(schemaname || '.' || tablename ORDER BY schemaname,tablename) "
            "FROM pg_tables WHERE schemaname IN ('public','management');"))
        before = fixture.snapshot(tables)
        backup = fixture.backup()
        fixture.restore(backup)
        assert fixture.snapshot(tables, database="restored") == before, "restored rows or baseline checksums differ"
        verify_schema(fixture, "restored")
        with fixture.server(SERVER, database="restored"):
            verify_login(fixture, owner)
            verify_reads(fixture, data)
            resume_pending(fixture, data)
            verify_reads(fixture, data, pending_state="active")
        assert not backend.vendor.list_multipart_uploads(Bucket=backend.spec["bucket"]).get("Uploads")
        print("PASS exact DB backup restoration, restart, existing URLs/keys and pending Native/S3 completion")


if __name__ == "__main__":
    run()
