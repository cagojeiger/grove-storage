#!/usr/bin/env python3
"""Run the real NoteGate UI/server against Grove with an isolated test issuer."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import socket
import subprocess
import tempfile
import time
import urllib.request

from notegate_oidc_fixture import oidc_fixture
from s3_backend_fixture import docker, minio_backend

ROOT = Path(__file__).resolve().parent
HARNESS = runpy.run_path(str(ROOT / "e2e-cli.py"))


def available_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def wait_ready(url):
    for _ in range(100):
        try:
            with urllib.request.urlopen(url, timeout=2) as response:
                if response.status == 200:
                    return
        except OSError:
            time.sleep(0.2)
    raise RuntimeError("local server readiness timeout")


def check(grove, directory, database, account, backend, notegate, origin, output):
    wait_ready(grove + "/readyz")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=notegate, text=True).strip()
    diff = subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=notegate)
    print("NoteGate revision:", revision, "working diff sha256:", hashlib.sha256(diff).hexdigest())

    from cli_management_fixture import Management
    management = Management(grove, account)
    management.command("storage.create", {"id": "browser-backend", "spec": backend.spec})
    management.command("client.create", {"id": "notegate-browser", "storage_id": "browser-backend"})
    credential = management.command("credential.create", {"client_id": "notegate-browser"})
    docker("exec", database, "createdb", "-U", "grove", "notegate_browser")
    port = docker("port", database, "5432").rsplit(":", 1)[1]
    with oidc_fixture(origin + "/auth/callback") as (issuer, events):
        env = {k: v for k, v in os.environ.items() if not k.startswith(("NOTEGATE_", "AWS_"))}
        env.update(NOTEGATE_DATABASE_URL=f"postgres://grove:grove@127.0.0.1:{port}/notegate_browser",
            NOTEGATE_BIND_ADDR=origin.removeprefix("http://"), NOTEGATE_PUBLIC_URL=origin,
            NOTEGATE_AUTHGATE_URL=issuer, NOTEGATE_OAUTH_CLIENT_ID="notegate-web",
            NOTEGATE_MCP_OAUTH_CLIENT_ID="notegate-mcp", NOTEGATE_CLI_OAUTH_CLIENT_ID="notegate-cli-local",
            NOTEGATE_ENC_ROOT_KEY_ID="browser-test-enc", NOTEGATE_ENC_ROOT_SECRET="browser-test-distinct-encryption-secret-32bytes",
            NOTEGATE_LOOKUP_ROOT_KEY_ID="browser-test-lookup", NOTEGATE_LOOKUP_ROOT_SECRET="browser-test-distinct-lookup-secret-32bytes",
            NOTEGATE_WEB_DIST_DIR=str(notegate / "frontend/web/dist"),
            NOTEGATE_S3__ENDPOINT=grove, NOTEGATE_S3__PUBLIC_ENDPOINT=grove,
            NOTEGATE_S3__REGION="us-east-1", NOTEGATE_S3__BUCKET="notegate-browser",
            NOTEGATE_S3__ACCESS_KEY=credential["access_key_id"], NOTEGATE_S3__SECRET_KEY=credential["secret_key"],
            NOTEGATE_S3__FORCE_PATH_STYLE="true", NO_PROXY="127.0.0.1,localhost")
        with (output / "notegate.log").open("wb") as log:
            server = subprocess.Popen([str(notegate / "target/debug/notegate-api")], cwd=directory,
                env=env, stdout=log, stderr=log)
            try:
                wait_ready(origin + "/")
                subprocess.run(["node", str(ROOT / "notegate_browser_flow.mjs"), str(notegate), origin,
                    grove, str(output)], check=True, timeout=300)
                assert events == ["authorize", "token_pkce_verified", "userinfo"], events
                assert subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=notegate) == diff, \
                    "NoteGate source changed during the browser test"
                (output / "context.json").write_text(json.dumps({"notegate_revision": revision,
                    "notegate_diff_sha256": hashlib.sha256(diff).hexdigest(),
                    "issuer": "loopback test fixture, not production AuthGate or Google",
                    "oidc_events": events, "transport": "loopback HTTP, not production TLS"}, indent=2))
                print("PASS OIDC authorization code, PKCE, signed ID token, and userinfo flow")
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--notegate-dir", required=True, type=Path)
    parser.add_argument("--output-dir", type=Path)
    options = parser.parse_args()
    notegate = options.notegate_dir.resolve()
    output = options.output_dir or Path(tempfile.mkdtemp(prefix="grove-browser-evidence-"))
    output.mkdir(parents=True, exist_ok=True)
    origin = f"http://127.0.0.1:{available_port()}"
    print("Evidence:", output)
    with minio_backend() as backend:
        HARNESS["main"](lambda grove, directory, database, account:
            check(grove, directory, database, account, backend, notegate, origin, output),
            with_database=True, s3_cors_origins=(origin,))
