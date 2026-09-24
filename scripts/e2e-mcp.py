#!/usr/bin/env python3
"""Real stateless MCP HTTP lifecycle, CLI parity, and secret-free server logs."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import time
import urllib.error
import urllib.request

from cli_management_fixture import Management

HARNESS = runpy.run_path(str(Path(__file__).with_name("e2e-cli.py")))
VERSION = "2026-07-28"
SECRETS = []


def check(endpoint, directory):
    from s3_backend_fixture import minio_backend
    with minio_backend() as backend:
        check_s3(endpoint, directory, backend)


def check_s3(endpoint, directory, backend):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    deadline = time.monotonic() + 20
    while True:
        try:
            with opener.open(endpoint + "/readyz", timeout=2) as response:
                if response.status == 200:
                    break
        except (urllib.error.URLError, TimeoutError):
            pass
        if time.monotonic() >= deadline:
            raise RuntimeError("MCP fixture readiness timeout")
        time.sleep(0.1)
    management = Management(endpoint, HARNESS["MASTER_TOKEN"])
    SECRETS.extend([HARNESS["MASTER_TOKEN"], management.token, management.user_token])
    calls = set()
    counter = 0

    def rpc(method, params, token=None):
        nonlocal counter
        counter += 1
        params = dict(params, _meta={"io.modelcontextprotocol/protocolVersion": VERSION,
                                    "io.modelcontextprotocol/clientCapabilities": {}})
        headers = {"Authorization": "Bearer " + (token or management.token),
                   "Content-Type": "application/json", "Accept": "application/json, text/event-stream",
                   "MCP-Protocol-Version": VERSION, "Mcp-Method": method}
        if "name" in params:
            headers["Mcp-Name"] = params["name"]
        request = urllib.request.Request(endpoint + "/api/admin/mcp", method="POST", headers=headers,
                                         data=json.dumps({"jsonrpc": "2.0", "id": counter,
                                                          "method": method, "params": params}).encode())
        with opener.open(request, timeout=10) as response:
            assert response.headers["Cache-Control"] == "no-store"
            assert response.headers["X-Content-Type-Options"] == "nosniff"
            assert not response.headers.get("Mcp-Session-Id")
            body = json.loads(response.read())
            assert body["id"] == counter
            assert "error" not in body, "MCP protocol failure"
            return body["result"]

    def call(name, arguments, error=None):
        reply = rpc("tools/call", {"name": name, "arguments": arguments})
        calls.add(name)
        assert reply["isError"] == (error is not None), name
        result = reply["structuredContent"]
        assert json.loads(reply["content"][0]["text"]) == result
        assert result["protocol"] == 1
        if error:
            assert result["error"] == {"code": error, "outcome": "not_applied"}
            return None
        assert result["command"] == name
        return result["result"]

    discovery = rpc("server/discover", {})
    assert VERSION in discovery["supportedVersions"]
    tools = rpc("tools/list", {})["tools"]
    names = {tool["name"] for tool in tools}
    assert len(names) == 19 and not any(name.startswith(("identity", "history")) for name in names)
    provider_secret = "provider-input-e2e-canary"
    SECRETS.append(provider_secret)
    call("storage.create", {"id": "incomplete-provider", "spec": {
        "kind": "s3", "capacity_bytes": 1024, "secret_key": provider_secret,
    }}, error="invalid_input")
    spec = dict(backend.spec)
    SECRETS.append(spec["secret_key"])
    for name in ("storage.create", "storage.replace"):
        try:
            rpc("tools/call", {"name": name, "arguments": {"id": "mcp-storage", "spec": {
                "kind": "fs", "root_path": "/not-probed", "capacity_bytes": 1,
            }}})
        except urllib.error.HTTPError as error:
            assert error.code == 400
            rejected = json.loads(error.read())["error"]
            assert rejected["code"] == -32602
            assert rejected["data"] == {"code": "invalid_input", "outcome": "not_applied"}
        else:
            raise AssertionError("MCP accepted filesystem registration")
    call("storage.create", {"id": "mcp-storage", "spec": spec})
    call("client.create", {"id": "mcp-client", "storage_id": "mcp-storage"})
    key = "sha256:" + hashlib.sha256(b"mcp-local-service-key").hexdigest()
    call("client-key.register", {"client_id": "mcp-client", "key_hash": key})
    issued = call("credential.create", {"client_id": "mcp-client"})
    SECRETS.append(issued["secret_key"])
    spec["capacity_bytes"] *= 2
    call("storage.replace", {"id": "mcp-storage", "spec": spec})
    env = {k: v for k, v in os.environ.items() if not k.startswith(("GROVE_", "FILEGATE_"))}
    env.update(GROVE_ENDPOINT=endpoint, GROVE_TOKEN=management.token, NO_PROXY="127.0.0.1")
    for name, arguments, args in [
        ("status", {}, ["status"]),
        ("storage.list", {}, ["storage", "list"]),
        ("storage.show", {"id": "mcp-storage"}, ["storage", "show", "mcp-storage"]),
        ("client.list", {}, ["client", "list"]),
        ("client.show", {"id": "mcp-client"}, ["client", "show", "mcp-client"]),
        ("client-key.list", {"client_id": "mcp-client"}, ["client-key", "list", "--client", "mcp-client"]),
        ("credential.list", {"client_id": "mcp-client"}, ["credential", "list", "--client", "mcp-client"]),
        ("usage.storages", {}, ["usage", "storages"]),
        ("usage.clients", {}, ["usage", "clients"]),
        ("usage.history", {"days": 7}, ["usage", "history", "--days", "7"]),
    ]:
        result = call(name, arguments)
        cli = subprocess.run([str(HARNESS["CLI"]), "--output", "json", *args],
                             env=env, cwd=directory, capture_output=True, text=True, timeout=10)
        assert cli.returncode == 0, name
        assert json.loads(cli.stdout)["data"] == result, name
        assert issued["secret_key"] not in cli.stdout + cli.stderr
    call("storage.delete", {"id": "mcp-storage"}, error="conflict")
    call("credential.delete", {"client_id": "mcp-client", "access_key_id": issued["access_key_id"]})
    call("client-key.delete", {"client_id": "mcp-client", "key_hash": key})
    call("client.delete", {"id": "mcp-client"})
    call("storage.delete", {"id": "mcp-storage"})
    assert calls == names
    audit = management.request("GET", "/history/audit?limit=100")
    events = [item for item in audit["items"] if item["context"].get("actor_id") == management.agent_id]
    assert len(events) == 9
    assert all(item["context"]["surface"] == "mcp" for item in events)
    assert all(secret not in json.dumps(audit) for secret in SECRETS)
    management.request("PATCH", f"/accounts/{management.user_id}", {"operation": "role", "role": "viewer"})
    call("client.create", {"id": "blocked", "storage_id": "missing"}, error="forbidden")
    call("client.list", {})
    assert not rpc("tools/call", {"name": "status", "arguments": {}}, management.user_token)["isError"]
    management.request("DELETE", f"/credentials/{management.credential_id}")
    try:
        rpc("tools/list", {})
    except urllib.error.HTTPError as error:
        assert error.code == 401
    else:
        raise AssertionError("Revoked token could discover tools")
    print("PASS MCP 19 commands, CLI parity, Agent audit, owner cap, revocation, and one-time secret")


def verify_log(data):
    text = data.decode(errors="replace")
    assert all(secret not in text for secret in SECRETS), "Secret appeared in server logs"
    assert "request.end" in text, "Application telemetry was unexpectedly suppressed"
    print("PASS server logs exclude management, service, and provider secrets with rmcp=trace")


if __name__ == "__main__":
    os.environ["RUST_LOG"] = "info,rmcp=trace"
    HARNESS["main"](check, management=True, verify_log=verify_log)
