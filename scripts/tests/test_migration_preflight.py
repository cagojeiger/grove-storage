import hashlib
import json
from pathlib import Path
import runpy
import sys
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
rehearsal = runpy.run_path(str(ROOT / "scripts/e2e-migration.py"))


class MigrationPreflightTests(unittest.TestCase):
    def rows(self):
        return [{"version": version, "success": True, "checksum": hashlib.sha384(
            next((ROOT / "backend/crates/db/migrations").glob(f"{version:04d}_*.sql")).read_bytes()
        ).hexdigest()} for version in range(1, 7)]

    def test_accepts_released_schema_with_no_recovery_owners(self):
        fixture = Mock()
        fixture.sql.side_effect = [json.dumps(self.rows()), "0", "0", "0"]
        rehearsal["preflight"](fixture, ROOT)
        self.assertEqual(fixture.sql.call_count, 4)

    def test_rejects_other_schema_failed_migration_and_checksum_drift(self):
        for failure in ("version", "success", "checksum"):
            with self.subTest(failure=failure):
                rows = self.rows()
                rows[-1][failure] = {"version": 7, "success": False, "checksum": "wrong"}[failure]
                fixture = Mock()
                fixture.sql.return_value = json.dumps(rows)
                with self.assertRaises(AssertionError):
                    rehearsal["preflight"](fixture, ROOT)

    def test_rejects_filesystem_and_inflight_completion_owners(self):
        for counts in (["1"], ["0", "1"], ["0", "0", "1"]):
            with self.subTest(counts=counts):
                fixture = Mock()
                fixture.sql.side_effect = [json.dumps(self.rows()), *counts]
                with self.assertRaises(AssertionError):
                    rehearsal["preflight"](fixture, ROOT)

    @patch("subprocess.check_output")
    def test_rejects_wrong_or_dirty_legacy_checkout_before_starting_services(self, output):
        for revision, dirty in (("wrong", ""), (rehearsal["LEGACY_REVISION"], " M Cargo.toml")):
            output.side_effect = [revision, dirty]
            with self.assertRaisesRegex(RuntimeError, "clean checkout"):
                rehearsal["legacy_binary"](ROOT)


if __name__ == "__main__":
    unittest.main()
