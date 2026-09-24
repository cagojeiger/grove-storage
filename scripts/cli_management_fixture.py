"""Console-only identity setup for the isolated CLI integration test."""
import json
import urllib.error
import urllib.request

ORIGIN = "https://console.test"
IDENTITY = "/api/admin/identity/v1"


class Management:
    def __init__(self, endpoint, master):
        self.endpoint = endpoint
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.cookie = None
        self.request("POST", "/master/session", {"token": master})
        owner = self.request("POST", "/master/bootstrap", {"display_name": "CLI test owner"}, expected=201)
        self.request("POST", "/session", {"token": owner["token"]})
        self.user_id = self.request("POST", "/accounts", {
            "kind": "user", "role": "writer", "display_name": "CLI writer",
        }, expected=201)["account_id"]
        personal = self.request("POST", f"/accounts/{self.user_id}/credentials", {
            "label": "cli-user", "expires_in_days": 1,
        }, expected=201)
        self.user_token, self.personal_credential_id = personal["token"], personal["credential_id"]
        issued = self.request("POST", f"/accounts/{self.user_id}/credentials", {
            "label": "cli-automation", "expires_in_days": 1,
        }, expected=201)
        self.token, self.credential_id = issued["token"], issued["credential_id"]

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
