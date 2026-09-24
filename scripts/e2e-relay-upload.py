#!/usr/bin/env python3
"""Single relay PUT races against a disposable API and PostgreSQL."""

import hashlib
import argparse
import http.client
import json
from pathlib import Path
import runpy
import time
import urllib.error
import urllib.parse
import urllib.request

HARNESS = runpy.run_path(str(Path(__file__).with_name("e2e-cli.py")))


def check(endpoint, directory, database, backend=None):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    key = "relay-regression-key"

    def request(method, path, body=None, token=key, expected=200):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(endpoint + path, data=data, method=method,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        with opener.open(req, timeout=10) as response:
            assert response.status == expected
            payload = response.read()
            return json.loads(payload) if payload else None

    def sql(query):
        return HARNESS["docker"]("exec", database, "psql", "-U", "filegate", "-d", "filegate",
                                "-v", "ON_ERROR_STOP=1", "-Atc", query)

    def put(url, body):
        try:
            with opener.open(urllib.request.Request(url, data=body, method="PUT"), timeout=10) as response:
                return response.status, response.headers.get("ETag")
        except urllib.error.HTTPError as error:
            return error.code, None

    deadline = time.monotonic() + 20
    while True:
        try:
            request("GET", "/readyz")
            break
        except (urllib.error.URLError, TimeoutError):
            if time.monotonic() > deadline:
                raise
            time.sleep(0.1)

    admin = HARNESS["TOKEN"]
    root = Path(directory) / "objects"
    root.mkdir()
    spec = ({"kind": "fs", "root_path": str(root), "capacity_bytes": 1000000}
            if backend is None else {**backend.spec, "force_relay": True})
    request("POST", "/api/admin/v1/storages", {"id": "relay", **spec}, admin, 201)
    request("POST", "/api/admin/v1/clients", {"id": "relay", "storage_id": "relay"}, admin, 201)
    request("POST", "/api/admin/v1/clients/relay/keys",
        {"key_hash": "sha256:" + hashlib.sha256(key.encode()).hexdigest()}, admin, 201)

    body = b"original"
    digest = hashlib.md5(body).hexdigest()

    def physical(file_id):
        object_key = sql(f"SELECT object_key FROM locations WHERE file_id = '{file_id}'")
        if backend is None:
            return (root / object_key).read_bytes()
        response = backend.vendor.get_object(Bucket=backend.spec["bucket"], Key=object_key)
        try:
            return response["Body"].read()
        finally:
            response["Body"].close()

    created = request("POST", "/api/v1/files", {"declared_size": len(body), "declared_md5": digest}, expected=201)
    file_id, url = created["file_id"], created["put_url"]
    sql(f"UPDATE leases SET expires_at = clock_timestamp() + interval '2 seconds' WHERE file_id = '{file_id}'")
    parsed = urllib.parse.urlsplit(url)
    connection = http.client.HTTPConnection(parsed.hostname, parsed.port, timeout=10)
    try:
        connection.putrequest("PUT", parsed.path + "?" + parsed.query)
        connection.putheader("Content-Length", str(len(body)))
        connection.endheaders()
        connection.send(body[:1])
        deadline = time.monotonic() + 5
        while sql("SELECT count(*) FROM pg_stat_activity WHERE state = 'idle in transaction' "
                  "AND query LIKE 'SELECT uploaded_size, uploaded_md5 FROM leases%'") == "0":
            assert time.monotonic() < deadline, "upload did not claim the lease"
            time.sleep(0.02)
        assert put(url, body)[0] == 409
        time.sleep(2.1)
        assert sql(f"SELECT state FROM files WHERE id = '{file_id}'") == "pending"
        connection.send(body[1:])
        response = connection.getresponse()
        assert response.status == 200, response.read()
        response.read()
    finally:
        connection.close()
    assert sql(f"SELECT expires_at > clock_timestamp() FROM leases WHERE file_id = '{file_id}'") == "t"
    assert put(url, body)[0] == 200
    assert put(url, b"modified")[0] == 400
    request("POST", f"/api/v1/files/{file_id}/commit")
    assert put(url, body)[0] == 403
    assert physical(file_id) == body
    print("PASS slow upload owns lease; competing PUT rejected; retry idempotent; committed bytes unchanged")

    created = request("POST", "/api/v1/files", {"declared_size": len(body)}, expected=201)
    url = created["put_url"]
    assert put(url, body)[0] == 200
    assert put(url, b"modified")[0] == 409
    assert put(url, body)[0] == 200
    assert physical(created["file_id"]) == body
    print("PASS changed retry cannot replace published bytes without a declared MD5")

    created = request("POST", "/api/v1/files", {"declared_size": len(body), "declared_md5": digest}, expected=201)
    assert put(created["put_url"], b"modified")[0] == 400
    assert put(created["put_url"], body)[0] == 200
    print("PASS failed checksum publishes no bytes and allows corrected retry")

    created = request("POST", "/api/v1/files", {"declared_size": len(body)}, expected=201)
    file_id, url = created["file_id"], created["put_url"]
    sql("""CREATE FUNCTION fail_relay_record() RETURNS trigger LANGUAGE plpgsql AS $$
           BEGIN IF NEW.uploaded_size IS NOT NULL THEN
             RAISE EXCEPTION 'injected relay measurement commit failure';
           END IF; RETURN NEW; END $$;
           CREATE CONSTRAINT TRIGGER fail_relay_record AFTER UPDATE ON leases
           DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_relay_record();""")
    try:
        assert put(url, body)[0] == 500
        assert physical(file_id) == body
        assert sql(f"SELECT uploaded_size IS NULL FROM leases WHERE file_id = '{file_id}'") == "t"
    finally:
        sql("DROP TRIGGER fail_relay_record ON leases; DROP FUNCTION fail_relay_record();")
    assert put(url, b"modified")[0] == 200
    assert physical(file_id) == b"modified"
    request("POST", f"/api/v1/files/{file_id}/commit")
    assert sql(f"SELECT etag FROM files WHERE id = '{file_id}'") == hashlib.md5(b"modified").hexdigest()
    print("PASS physical success with DB commit failure remains unmeasured and permits consistent retry")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend", choices=["fs", "minio"], default="fs")
    args = parser.parse_args()
    if args.backend == "minio":
        from s3_backend_fixture import minio_backend
        with minio_backend() as backend:
            HARNESS["main"](lambda endpoint, directory, database: check(endpoint, directory, database, backend),
                            with_database=True, reconciler_interval=3600)
    else:
        HARNESS["main"](check, with_database=True, reconciler_interval=3600)
