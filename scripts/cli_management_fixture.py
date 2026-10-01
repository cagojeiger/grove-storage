"""Console-only identity setup for the isolated CLI integration test."""
import json
import os
from pathlib import Path
import secrets
import subprocess
import urllib.error
import urllib.request

ORIGIN = "https://console.test"
IDENTITY = "/api/admin/identity/v1"


class Management:
    def __init__(self, endpoint, database):
        self.endpoint = endpoint
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.cookie = None
        owner_password = "fixture-" + secrets.token_urlsafe(32)
        subprocess.check_output(
            ["python3", "scripts/e2e-password-account.py", database],
            cwd=Path(__file__).resolve().parent.parent,
            env=dict(os.environ, GROVE_E2E_PASSWORD=owner_password,
                     GROVE_E2E_DISPLAY_NAME="CLI test owner"), text=True, timeout=45)
        self.request("POST", "/session", {"username": "owner", "password": owner_password})
        setup = self.request("POST", "/accounts", {
            "kind": "user_with_password_setup", "role": "writer", "display_name": "CLI writer",
            "username": "cli-writer", "current_password": owner_password,
        }, expected=201)
        self.user_id = setup["account_id"]
        writer_password = "fixture-" + secrets.token_urlsafe(32)
        self.request("POST", "/password-setup", {
            "token": setup["token"], "password": writer_password,
        }, expected=204)
        self.request("POST", "/session", {"username": "cli-writer", "password": writer_password})
        personal = self.request("POST", "/me/tokens", {
            "label": "cli-user", "expires_in_days": 1, "current_password": writer_password,
        }, expected=201)
        self.user_token, self.personal_credential_id = personal["token"], personal["credential_id"]
        issued = self.request("POST", "/me/tokens", {
            "label": "cli-automation", "expires_in_days": 1, "current_password": writer_password,
        }, expected=201)
        self.token, self.credential_id = issued["token"], issued["credential_id"]
        self.request("POST", "/session", {"username": "owner", "password": owner_password})

    def request(self, method, path, body=None, expected=200):
        headers = {"Origin": ORIGIN, "X-Grove-CSRF": "1", "Content-Type": "application/json"}
        if self.cookie:
            headers["Cookie"] = self.cookie
        request = urllib.request.Request(self.endpoint + IDENTITY + path, method=method,
                                         headers=headers, data=None if body is None else json.dumps(body).encode())
        with self.opener.open(request, timeout=5) as response:
            assert response.status == expected, (path, response.status, expected)
            cookie = response.headers.get("Set-Cookie")
            if cookie:
                self.cookie = cookie.split(";", 1)[0]
            data = response.read()
            return json.loads(data) if data else None

    def verify(self, run):
        run(["status"], self.user_token, 0)
        for path in ["/accounts", "/history/audit"]:
            request = urllib.request.Request(self.endpoint + IDENTITY + path,
                                             headers={"Authorization": "Bearer " + self.token})
            try:
                self.opener.open(request, timeout=5)
            except urllib.error.HTTPError as error:
                assert error.code in (401, 403)
            else:
                raise AssertionError("User bearer entered console-only identity boundary")
        audit = self.request("GET", "/history/audit?limit=100")
        events = [item for item in audit["items"] if item["context"].get("credential_id") == self.credential_id]
        assert events and any(item["action"] == "storage.create" for item in events)
        assert all(item["context"]["surface"] == "resource_api" for item in events)
        assert all(item["context"]["actor_id"] == self.user_id for item in events)
        calls = self.request("GET", "/history/invocations?limit=100")
        token_ids = {item["context"]["credential_id"] for item in calls["items"]
                     if item["context"].get("actor_id") == self.user_id}
        assert {self.personal_credential_id, self.credential_id} <= token_ids
        text = json.dumps(audit)
        assert self.token not in text and self.user_token not in text
        self.request("PATCH", f"/accounts/{self.user_id}", {"operation": "role", "role": "reader"})
        run(["client", "list"], self.token, 0)
        run(["client", "create", "blocked", "--storage", "cli-test-fs"], self.token, 3)
        self.request("DELETE", f"/credentials/{self.credential_id}")
        run(["status"], self.token, 3)
        run(["status"], self.user_token, 0)
        print("PASS real named User token CLI authentication, console isolation, audit actor, User role, and revocation")
