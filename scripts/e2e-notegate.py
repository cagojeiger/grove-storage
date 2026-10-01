#!/usr/bin/env python3
"""Run NoteGate's real upload handlers and Rust S3 SDK against disposable Grove."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import subprocess
import time
import urllib.error
import urllib.request

from s3_backend_fixture import docker, minio_backend


HARNESS = runpy.run_path(str(Path(__file__).with_name("e2e-cli.py")))


def check(endpoint, directory, database, backend, notegate):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def admin(method, path, body=None):
        request = urllib.request.Request(endpoint + path, method=method,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Authorization": "Bearer " + HARNESS["TOKEN"],
                     "Content-Type": "application/json"})
        with opener.open(request, timeout=5) as response:
            return json.load(response)

    deadline = time.monotonic() + 20
    while True:
        try:
            if admin("GET", "/readyz") == {"status": "ready"}:
                break
        except (OSError, urllib.error.URLError):
            pass
        if time.monotonic() >= deadline:
            raise RuntimeError("Grove readiness timeout")
        time.sleep(0.1)

    admin("POST", "/api/admin/v1/storages", {"id": "notegate-backend", **backend.spec})
    admin("POST", "/api/admin/v1/clients", {"id": "notegate-contract", "storage_id": "notegate-backend"})
    credential = admin("POST", "/api/admin/v1/clients/notegate-contract/s3-credentials", {})
    preflight = urllib.request.Request(endpoint + "/notegate-contract/cors-probe", method="OPTIONS",
        headers={"Origin": "http://localhost:5173", "Access-Control-Request-Method": "PUT",
                 "Access-Control-Request-Headers": "content-type,if-none-match"})
    with opener.open(preflight, timeout=5) as response:
        assert response.headers["Access-Control-Allow-Origin"] == "http://localhost:5173"
        allowed = response.headers["Access-Control-Allow-Headers"].lower().split(',')
        assert {"content-type", "if-none-match"} <= {header.strip() for header in allowed}
    print("PASS NoteGate browser origin and conditional PUT CORS preflight")
    # The fixture owns this entire container. NoteGate uses a separate database.
    docker("exec", database, "createdb", "-U", "filegate", "notegate_contract")
    db_port = docker("port", database, "5432").rsplit(":", 1)[1]
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("NOTEGATE_", "AWS_"))}
    env.update(
        NOTEGATE_TEST_DATABASE_URL=f"postgres://filegate:filegate@127.0.0.1:{db_port}/notegate_contract",
        NOTEGATE_TEST_S3_ENDPOINT=endpoint,
        NOTEGATE_TEST_S3_PUBLIC_ENDPOINT=endpoint,
        NOTEGATE_TEST_S3_REGION="us-east-1",
        NOTEGATE_TEST_S3_BUCKET="notegate-contract",
        NOTEGATE_TEST_S3_ACCESS_KEY=credential["access_key_id"],
        NOTEGATE_TEST_S3_SECRET_KEY=credential["secret_key"],
        NO_PROXY="127.0.0.1,localhost",
    )
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=notegate, text=True).strip()
    diff = subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=notegate)
    print("NoteGate revision:", revision, "working diff sha256:", hashlib.sha256(diff).hexdigest())
    result = subprocess.run(
        ["cargo", "test", "-p", "notegate-api", "rest::file_upload_tests::", "--locked",
         "--", "--test-threads=1", "--nocapture"],
        cwd=notegate, env=env, text=True, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, timeout=1800,
    )
    print(result.stdout)
    assert result.returncode == 0, "NoteGate upload contracts failed against Grove"
    assert "skipping Postgres tests" not in result.stdout, "database tests were skipped"
    assert re.search(r"running [1-9][0-9]* tests", result.stdout), "no upload tests ran"
    assert subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=notegate) == diff, \
        "NoteGate working source changed during the test; repeat against a stable snapshot"
    print("PASS actual NoteGate REST/MCP handlers and Rust S3 SDK against Grove; source unchanged")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--notegate-dir", required=True, type=Path)
    options = parser.parse_args()
    notegate = options.notegate_dir.resolve()
    if not (notegate / "backend/crates/api/src/rest/file_upload_tests.rs").is_file():
        parser.error("--notegate-dir must contain NoteGate's file upload contract suite")
    with minio_backend() as backend:
        HARNESS["main"](lambda endpoint, directory, database:
            check(endpoint, directory, database, backend, notegate), with_database=True,
            s3_cors_origins=("http://localhost:5173",))
