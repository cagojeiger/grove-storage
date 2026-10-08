#!/usr/bin/env python3
"""Exercise the gscli registry lifecycle against a disposable Grove Storage API."""

import hashlib
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid


ROOT = Path(__file__).resolve().parent.parent
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
SERVER = TARGET / "debug" / "grove-storage"
CLI = TARGET / "debug" / "gscli"


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True, timeout=90).strip()


def check_lifecycle(endpoint, directory, database, account):
    from s3_backend_fixture import minio_backend
    with minio_backend() as backend:
        check_s3_lifecycle(endpoint, directory, backend, account)


def check_s3_lifecycle(endpoint, directory, backend, account):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def request(method, path, body=None, token=""):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(
            endpoint + path, data=data, method=method,
            headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
        )
        with opener.open(req, timeout=5) as response:
            payload = response.read()
            return None if not payload else json.loads(payload)

    deadline = time.monotonic() + 20
    while True:
        try:
            if request("GET", "/readyz") == {"status": "ready"}:
                break
        except (urllib.error.URLError, TimeoutError):
            pass
        if time.monotonic() >= deadline:
            raise RuntimeError("Test server did not become ready")
        time.sleep(0.1)

    from cli_management_fixture import Management
    management = Management(endpoint, account)
    env = {k: v for k, v in os.environ.items() if not k.startswith(("GROVE_",))}
    env.pop("DATABASE_URL", None)
    env.update(GROVE_ENDPOINT=endpoint, GROVE_TOKEN=management.token, NO_PROXY="127.0.0.1")

    def run_cli(*args):
        result = subprocess.run(
            [str(CLI), "--output", "json", "--timeout", "5", *args],
            cwd=directory, env=env, capture_output=True, text=True, timeout=10,
        )
        assert result.returncode == 0, (args, result.stdout, result.stderr)
        assert not result.stderr
        output = json.loads(result.stdout)
        assert output["ok"] and output["error"] is None
        assert management.token not in result.stdout
        print("PASS", " ".join(args))
        return output, result

    storage_spec = Path(directory) / "storage.json"
    storage_spec.write_text(json.dumps({
        **backend.spec, "capacity_bytes": 1073741824,
    }))
    run_cli("storage", "create", "cli-test-s3", "--from", str(storage_spec))
    checked, _ = run_cli("storage", "test", "cli-test-s3")
    assert checked["data"] == {"id": "cli-test-s3", "state": "ok"}
    run_cli("client", "create", "cli-test", "--storage", "cli-test-s3")

    raw_key = "cli-test-native-key"
    key_file = Path(directory) / "client-key"
    key_file.write_text(raw_key + "\n")
    key = "sha256:" + hashlib.sha256(raw_key.encode()).hexdigest()
    output, result = run_cli(
        "client-key", "register", "--client", "cli-test", "--key-file", str(key_file),
    )
    assert output["data"]["key_hash"] == key
    assert raw_key not in result.stdout

    secret_file = Path(directory) / "s3-credential.json"
    output, result = run_cli(
        "credential", "create", "--client", "cli-test", "--secret-out", str(secret_file),
    )
    credential = json.loads(secret_file.read_text())
    assert output["data"]["access_key_id"] == credential["access_key_id"]
    assert output["data"]["file_state"] == "saved"
    assert credential["secret_key"] not in result.stdout
    assert secret_file.stat().st_mode & 0o777 == 0o600

    storage_spec.write_text(json.dumps({
        **backend.spec, "capacity_bytes": 2147483648,
    }))
    run_cli("storage", "replace", "cli-test-s3", "--from", str(storage_spec), "--yes")

    cases = [
        (["status"], None, {}),
        (["storage", "list"], "storage.list", {}),
        (["storage", "show", "cli-test-s3"], "storage.show", {"id": "cli-test-s3"}),
        (["client", "list"], "client.list", {}),
        (["client", "show", "cli-test"], "client.show", {"id": "cli-test"}),
        (["credential", "list", "--client", "cli-test"], "credential.list", {"client_id": "cli-test"}),
        (["client-key", "list", "--client", "cli-test"], "client-key.list", {"client_id": "cli-test"}),
        (["usage", "storages"], "usage.storages", {}),
        (["usage", "clients"], "usage.clients", {}),
        (["usage", "history", "--days", "7"], "usage.history", {"days": 7}),
    ]
    for args, command, inputs in cases:
        output, result = run_cli(*args)
        assert credential["secret_key"] not in result.stdout
        if command is not None:
            assert output["data"] == management.command(command, inputs), args
        else:
            assert output["data"]["registry"]["storage_count"] == 1
            assert output["data"]["registry"]["client_count"] == 1
            assert output["data"]["storage_access"] == "not_checked"

    access_key_id = credential["access_key_id"]
    run_cli("credential", "delete", "--client", "cli-test", access_key_id, "--yes")
    run_cli("client-key", "delete", "--client", "cli-test", key, "--yes")
    run_cli("client", "delete", "cli-test", "--yes")
    run_cli("storage", "delete", "cli-test-s3", "--yes")
    assert management.command("client.list") == []
    assert management.command("storage.list") == []
    print("PASS registry lifecycle completed without Terraform")

    # A separate fixture remains in the disposable database until container teardown.
    run_cli("storage", "create", "cli-test-s3", "--from", str(storage_spec))
    run_cli("client", "create", "cli-test", "--storage", "cli-test-s3")
    run_cli("client-key", "register", "--client", "cli-test", "--key-file", str(key_file))
    request("POST", "/api/v1/files", {"declared_size": 0}, token=raw_key)
    run_cli("storage", "replace", "cli-test-s3", "--from", str(storage_spec), "--yes")
    replacement = {**backend.spec, "public_endpoint": backend.spec["endpoint"] + "/changed",
                   "capacity_bytes": 2147483648}
    try:
        management.command("storage.replace", {"id": "cli-test-s3", "spec": replacement})
    except urllib.error.HTTPError as error:
        assert error.code == 409, error.code
    else:
        raise AssertionError("storage address replacement should be rejected")
    storage_spec.write_text(json.dumps(replacement))
    rejected = subprocess.run(
        [str(CLI), "--output", "json", "storage", "replace", "cli-test-s3",
         "--from", str(storage_spec), "--yes"],
        cwd=directory, env=env, capture_output=True, text=True, timeout=10,
    )
    assert rejected.returncode != 0, rejected.stdout
    assert management.command("storage.show", {"id": "cli-test-s3"})["public_endpoint"] == backend.spec["endpoint"]
    print("PASS API 409 and CLI rejection preserve a storage with pending files")

    def run_as(args, token, expected):
        result = subprocess.run([str(CLI), "--output", "json", *args],
                                cwd=directory, env=dict(env, GROVE_TOKEN=token),
                                capture_output=True, text=True, timeout=10)
        assert result.returncode == expected, (args, result.returncode, expected)
        assert token not in result.stdout + result.stderr
        return json.loads(result.stdout)
    management.verify(run_as)


def main(check=check_lifecycle, *, with_database=False, with_restart=False, console_origin=None,
         reconciler_interval=1, verify_log=None, multipart=False,
         s3_cors_origins=(), database_url_transform=None):
    if with_restart and not with_database:
        raise ValueError("restart checks require the isolated database fixture")
    if not SERVER.is_file() or not CLI.is_file():
        raise RuntimeError("Run cargo build --bin grove-storage --bin gscli --locked first")
    container = "grove-cli-e2e-" + uuid.uuid4().hex[:12]
    try:
        docker("run", "--rm", "-d", "--name", container,
               "-e", "POSTGRES_USER=grove", "-e", "POSTGRES_PASSWORD=grove",
               "-e", "POSTGRES_DB=grove", "-p", "127.0.0.1::5432", "postgres:17-alpine")
        db_port = docker("port", container, "5432").rsplit(":", 1)[1]
        with tempfile.TemporaryDirectory(prefix="grove-cli-e2e-") as directory:
            with socket.socket() as listener:
                listener.bind(("127.0.0.1", 0))
                port = listener.getsockname()[1]
            endpoint = f"http://127.0.0.1:{port}"
            env = {k: v for k, v in os.environ.items() if not k.startswith("GROVE_")}
            env.update(
                GROVE_DATABASE_URL=f"postgres://grove:grove@127.0.0.1:{db_port}/grove",
                GROVE_ENC_ROOT_SECRET="local-cli-integration-root-secret-32bytes",
                GROVE_BIND=f"127.0.0.1:{port}",
                GROVE_CONSOLE_ORIGIN=console_origin or "https://console.test",
                GROVE_PUBLIC_URL=endpoint, GROVE_LOG_FORMAT="json",
            )
            if with_database:
                env["GROVE_RECONCILER_INTERVAL_SECS"] = str(reconciler_interval)
            if multipart:
                env.update(GROVE_MULTIPART_THRESHOLD_BYTES=str(6 * 1024 * 1024),
                           GROVE_PART_SIZE_BYTES=str(5 * 1024 * 1024))
            if s3_cors_origins:
                env["GROVE_S3_CORS_ALLOWED_ORIGINS"] = ",".join(s3_cors_origins)
            deadline = time.monotonic() + 20
            while subprocess.run(["docker", "exec", container, "pg_isready", "-h", "127.0.0.1", "-U", "grove"],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5).returncode:
                if time.monotonic() >= deadline:
                    raise RuntimeError("Test database did not become ready")
                time.sleep(0.2)
            from cli_management_fixture import initialize_owner
            account = initialize_owner(container)
            if database_url_transform:
                env["GROVE_DATABASE_URL"] = database_url_transform(env["GROVE_DATABASE_URL"])
            with tempfile.TemporaryFile() as log:
                server = subprocess.Popen([str(SERVER)], env=env, cwd=directory, stdout=log, stderr=log)

                def crash_and_restart():
                    nonlocal server
                    assert server.poll() is None, "fixture server already exited"
                    previous_pid = server.pid
                    server.kill()
                    assert server.wait(timeout=5) == -signal.SIGKILL
                    server = subprocess.Popen([str(SERVER)], env=env, cwd=directory, stdout=log, stderr=log)
                    assert server.pid != previous_pid
                    print("PASS SIGKILL confirmed; new process started with the same DB and endpoint")

                try:
                    verify_modern_startup(endpoint)
                    if with_restart:
                        check(endpoint, directory, container, account, crash_and_restart)
                    elif with_database:
                        check(endpoint, directory, container, account)
                    else:
                        check(endpoint, directory, account)
                finally:
                    server.terminate()
                    try:
                        server.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait(timeout=5)
                if verify_log:
                    log.seek(0)
                    verify_log(log.read())
    finally:
        subprocess.run(["docker", "rm", "-f", container],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30, check=True)
    print("PASS disposable server, database, and files cleaned up")


def verify_modern_startup(endpoint):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    deadline = time.monotonic() + 20
    while True:
        try:
            with opener.open(endpoint + "/readyz", timeout=2) as response:
                if json.load(response) == {"status": "ready"}:
                    break
        except OSError:
            pass
        if time.monotonic() >= deadline:
            raise RuntimeError("Account-initialized test server did not become ready")
        time.sleep(0.1)
    try:
        with opener.open(endpoint + "/api/admin/v1/clients", timeout=5):
            raise AssertionError("legacy management must be disabled")
    except urllib.error.HTTPError as error:
        assert error.code == 410
        assert json.load(error) == {"error": "legacy_admin_removed"}
    print("PASS local Account bootstrap; legacy management disabled (410)")


if __name__ == "__main__":
    main(with_database=True)
