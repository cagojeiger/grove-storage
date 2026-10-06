#!/usr/bin/env python3
"""Verify a built image using only disposable Docker resources."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
import urllib.request
import uuid


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True, timeout=120).strip()


def check(image):
    suffix = uuid.uuid4().hex[:12]
    network, database, server = (f"grove-image-{kind}-{suffix}" for kind in ["net", "db", "api"])
    token, client_key = uuid.uuid4().hex, uuid.uuid4().hex
    version = (Path(__file__).resolve().parent.parent / "VERSION").read_text().strip()
    docker("network", "create", network)
    try:
        docker("run", "--rm", "-d", "--name", database, "--network", network,
               "-e", "POSTGRES_USER=filegate", "-e", "POSTGRES_PASSWORD=image-test",
               "-e", "POSTGRES_DB=filegate", "postgres:17-alpine")
        deadline = time.monotonic() + 30
        while subprocess.run(["docker", "exec", database, "pg_isready", "-h", "127.0.0.1", "-U", "filegate"],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5).returncode:
            if time.monotonic() > deadline:
                raise RuntimeError("image test PostgreSQL readiness timeout")
            time.sleep(0.2)
        docker("run", "--rm", "-d", "--name", server, "--network", network,
               "--read-only", "--tmpfs", "/tmp:rw,nosuid,nodev,uid=10001,gid=10001",
               "-p", "127.0.0.1::8080",
               "-e", f"FILEGATE_DATABASE_URL=postgres://filegate:image-test@{database}:5432/filegate",
               "-e", "FILEGATE_ENC_ROOT_SECRET=" + uuid.uuid4().hex,
               "-e", "FILEGATE_OPERATOR_TOKENS=" + token,
               "-e", "FILEGATE_PUBLIC_URL=http://127.0.0.1:8080", image)
        port = docker("port", server, "8080").rsplit(":", 1)[1]
        endpoint = "http://127.0.0.1:" + port
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

        def request(method, path, body=None, auth=token, expected=200):
            req = urllib.request.Request(endpoint + path, method=method,
                data=None if body is None else json.dumps(body).encode(),
                headers={"Authorization": "Bearer " + auth, "Content-Type": "application/json"})
            with opener.open(req, timeout=10) as response:
                assert response.status == expected
                payload = response.read()
                return json.loads(payload) if payload else None

        deadline = time.monotonic() + 30
        while True:
            try:
                assert request("GET", "/readyz") == {"status": "ready"}
                break
            except OSError:
                if time.monotonic() > deadline:
                    raise RuntimeError("image test API readiness timeout") from None
                time.sleep(0.2)
        assert docker("exec", server, "id", "-u") == "10001"
        assert request("GET", "/healthz") == {"status": "ok"}
        docker("exec", server, "mkdir", "/tmp/objects")
        request("POST", "/api/admin/v1/storages", {"id": "image", "kind": "fs",
            "root_path": "/tmp/objects", "capacity_bytes": 1048576}, expected=201)
        request("POST", "/api/admin/v1/clients", {"id": "image", "storage_id": "image"}, expected=201)
        request("POST", "/api/admin/v1/clients/image/keys",
            {"key_hash": "sha256:" + hashlib.sha256(client_key.encode()).hexdigest()}, expected=201)
        body = b"grove-image-contract"
        created = request("POST", "/api/v1/files", {"declared_size": len(body)}, auth=client_key, expected=201)
        put_url = created["put_url"].replace("http://127.0.0.1:8080", endpoint, 1)
        with opener.open(urllib.request.Request(put_url, data=body, method="PUT"), timeout=10) as response:
            assert response.status == 200
        file_id = created["file_id"]
        request("POST", f"/api/v1/files/{file_id}/commit", auth=client_key)
        read = request("POST", f"/api/v1/files/{file_id}/read", {}, auth=client_key)
        get_url = read["get_url"].replace("http://127.0.0.1:8080", endpoint, 1)
        with opener.open(get_url, timeout=10) as response:
            assert response.read() == body
        output = docker("exec", server, "/usr/local/bin/filegate", "status")
        assert f"filegate {version}   db ok" in output, output
        assert request("DELETE", f"/api/v1/files/{file_id}", auth=client_key) == {
            "file_id": file_id, "state": "deleted"}
        print(f"PASS image {image}: version {version}, UID 10001, read-only root, DB migration, probes, upload/read/delete")
    finally:
        for name in [server, database]:
            subprocess.run(["docker", "rm", "-f", "-v", name],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
        docker("network", "rm", network)


if __name__ == "__main__":
    check(sys.argv[1])
