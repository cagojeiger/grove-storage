import hashlib
import json
from pathlib import Path
import runpy
import sys
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
verification = runpy.run_path(str(ROOT / "scripts/e2e-installation.py"))
import installation_fixture


class InstallationTests(unittest.TestCase):
    def baseline_rows(self):
        return [{
            "version": int(source.name.split("_", 1)[0]), "success": True,
            "checksum": hashlib.sha384(source.read_bytes()).hexdigest(),
        } for source in sorted((ROOT / "backend/crates/db/migrations").glob("*.sql"))]

    def test_matches_current_baseline_and_account_schema(self):
        fixture = Mock()
        fixture.sql.side_effect = [json.dumps(self.baseline_rows()), "0", "0", "NO"]
        verification["verify_schema"](fixture)
        self.assertEqual(fixture.sql.call_count, 4)

    def test_rejects_stale_baseline_failed_migration_and_checksum_drift(self):
        current = self.baseline_rows()
        for rows in [
            current[:-1],
            [*current[:-1], {**current[-1], "success": False}],
            [*current[:-1], {**current[-1], "checksum": "wrong"}],
        ]:
            with self.subTest(rows=rows):
                fixture = Mock()
                fixture.sql.return_value = json.dumps(rows)
                with self.assertRaisesRegex(AssertionError, "does not match"):
                    verification["verify_schema"](fixture)

    def test_rejects_obsolete_schema_or_nullable_session_owner(self):
        for state in [["1"], ["0", "1"], ["0", "0", "YES"], ["0", "0", ""]]:
            with self.subTest(state=state):
                fixture = Mock()
                fixture.sql.side_effect = [json.dumps(self.baseline_rows()), *state]
                with self.assertRaises(AssertionError):
                    verification["verify_schema"](fixture)

    @patch("installation_fixture.subprocess.run")
    @patch("installation_fixture.docker", side_effect=["container-id", "127.0.0.1:54321"])
    def test_database_readiness_requires_tcp_not_initialization_socket(self, docker, run):
        run.return_value.returncode = 0
        with installation_fixture.installation() as fixture:
            self.assertEqual(run.call_args.args[0], ["docker", "exec", fixture.container,
                "pg_isready", "-h", "127.0.0.1", "-U", "grove"])
        self.assertEqual(run.call_args.args[0][:4], ["docker", "rm", "-f", "-v"])

    @patch("installation_fixture.subprocess.Popen")
    def test_startup_failure_includes_diagnostic_and_reaps_process(self, popen):
        process = Mock()
        process.poll.return_value = 1

        def start(*args, **kwargs):
            kwargs["stderr"].write(b"fixture startup diagnostic")
            return process

        popen.side_effect = start
        fixture = installation_fixture.InstallationFixture("unused", ROOT, "54321")
        with self.assertRaisesRegex(RuntimeError, "fixture startup diagnostic"):
            with fixture.server(Path("unused")):
                self.fail("a failed server must not become ready")
        process.wait.assert_called_once_with(timeout=10)

    @patch.dict("os.environ", {"GROVE_OPERATOR_TOKENS": "external-secret", "GROVE_LEGACY_ADMIN_ENABLED": "true"})
    def test_fixture_uses_new_configuration_and_drops_external_authentication(self):
        fixture = installation_fixture.InstallationFixture("unused", ROOT, "54321")
        env = fixture.environment()
        self.assertNotIn("GROVE_LEGACY_ADMIN_ENABLED", env)
        self.assertNotIn("GROVE_OPERATOR_TOKENS", env)
        self.assertIn("grove:grove@127.0.0.1:54321/grove", env["GROVE_DATABASE_URL"])


if __name__ == "__main__":
    unittest.main()
