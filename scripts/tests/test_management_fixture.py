import io
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import MagicMock
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from cli_management_fixture import Management


class ManagementCommandTests(unittest.TestCase):
    def setUp(self):
        self.management = Management.__new__(Management)
        self.management.endpoint = "http://127.0.0.1:12345"
        self.management.token = "fixture-management-token"
        self.management.cookie = "console-session=must-not-be-sent"
        self.management.opener = MagicMock()

    def respond(self, **overrides):
        payload = {"protocol": 1, "command": "client.list", "result": [],
                   "request_id": "ed54cc44-80bc-4aaa-8573-d2713dc9e240", **overrides}
        response = self.management.opener.open.return_value.__enter__.return_value
        response.status = 200
        response.headers = {"Cache-Control": "no-store"}
        response.read.return_value = json.dumps(payload).encode()

    def test_bearer_command_does_not_mix_console_cookie(self):
        self.respond()
        self.assertEqual(self.management.command("client.list"), [])
        request = self.management.opener.open.call_args.args[0]
        self.assertEqual(request.full_url, self.management.endpoint + "/api/admin/commands/v1")
        self.assertEqual(request.method, "POST")
        self.assertEqual(request.get_header("Authorization"), "Bearer " + self.management.token)
        self.assertFalse(request.has_header("Cookie"))
        self.assertEqual(json.loads(request.data), {"protocol": 1, "command": "client.list", "input": {}})

    def test_create_keeps_nested_storage_spec(self):
        self.respond(command="storage.create", result={"id": "archive"})
        inputs = {"id": "archive", "spec": {"kind": "s3", "capacity_bytes": 1024}}
        self.assertEqual(self.management.command("storage.create", inputs), {"id": "archive"})
        request = self.management.opener.open.call_args.args[0]
        self.assertEqual(json.loads(request.data)["input"], inputs)

    def test_rejects_wrong_protocol_or_command(self):
        for overrides in ({"protocol": 2}, {"command": "storage.list"}):
            with self.subTest(overrides=overrides):
                self.respond(**overrides)
                with self.assertRaises(AssertionError):
                    self.management.command("client.list")

    def test_requires_request_identity(self):
        self.respond(request_id="not-a-uuid")
        with self.assertRaises(ValueError):
            self.management.command("client.list")

    def test_propagates_http_failure_without_legacy_fallback(self):
        error = urllib.error.HTTPError(self.management.endpoint, 409, "Conflict", {}, io.BytesIO(b"{}"))
        self.management.opener.open.side_effect = error
        with self.assertRaises(urllib.error.HTTPError) as raised:
            self.management.command("storage.replace", {"id": "archive", "spec": {}})
        self.assertIs(raised.exception, error)
        self.management.opener.open.assert_called_once()


if __name__ == "__main__":
    unittest.main()
