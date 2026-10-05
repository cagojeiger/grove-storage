import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("image_security", ROOT / "deploy/ci/image-security.py")
security = importlib.util.module_from_spec(spec)
spec.loader.exec_module(security)


def clean_report():
    return {
        "SchemaVersion": 2,
        "Results": [{"Type": "rustbinary", "Packages": [{"Name": "tokio", "Version": "1.0.0"}]}],
    }


class ImageSecurityTests(unittest.TestCase):
    def test_clean_rust_inventory_passes(self):
        security.validate(clean_report())

    def test_missing_or_malformed_evidence_fails(self):
        for report in (None, {}, {"SchemaVersion": 1}, {"SchemaVersion": 2},
                       {"SchemaVersion": 2, "Results": []},
                       {"SchemaVersion": 2, "Results": [None]}):
            with self.subTest(report=report), self.assertRaises(ValueError):
                security.validate(report)

    def test_os_only_report_cannot_hide_missing_rust_inventory(self):
        with self.assertRaises(ValueError):
            security.validate({"SchemaVersion": 2, "Results": [{"Type": "debian"}]})

    def test_missing_rust_package_fields_fail(self):
        for packages in (None, [], {}, [None], [{"Name": "tokio"}]):
            report = clean_report()
            report["Results"][0]["Packages"] = packages
            with self.subTest(packages=packages), self.assertRaises(ValueError):
                security.validate(report)

    def test_high_critical_vulnerabilities_and_secrets_fail_even_without_a_fix(self):
        for kind in ("Vulnerabilities", "Secrets"):
            for severity in ("HIGH", "CRITICAL"):
                report = clean_report()
                report["Results"][0][kind] = [{"Severity": severity}]
                with self.subTest(kind=kind, severity=severity), self.assertRaises(ValueError):
                    security.validate(report)

    def test_lower_severity_and_null_findings_pass(self):
        report = clean_report()
        report["Results"][0]["Vulnerabilities"] = [{"Severity": "MEDIUM"}]
        report["Results"][0]["Secrets"] = None
        security.validate(report)

    def test_malformed_findings_fail(self):
        for entries in ("invalid", {}, False, [None], [{}], [{"Severity": "unexpected"}]):
            report = clean_report()
            report["Results"][0]["Secrets"] = entries
            with self.subTest(entries=entries), self.assertRaises(ValueError):
                security.validate(report)

    def test_both_platforms_resolve_without_attestation_manifests(self):
        index = {"manifests": [
            {"platform": {"os": "linux", "architecture": architecture},
             "digest": "sha256:" + character * 64}
            for architecture, character in (("amd64", "a"), ("arm64", "b"), ("unknown", "c"))
        ]}
        self.assertEqual(security.platform_digest(index, "amd64"), "sha256:" + "a" * 64)
        self.assertEqual(security.platform_digest(index, "arm64"), "sha256:" + "b" * 64)

    def test_missing_duplicate_invalid_or_non_linux_platform_fails(self):
        descriptor = {"platform": {"os": "linux", "architecture": "arm64"}, "digest": "sha256:" + "a" * 64}
        non_linux = copy.deepcopy(descriptor)
        non_linux["platform"]["os"] = "windows"
        invalid = copy.deepcopy(descriptor)
        invalid["digest"] = "latest"
        for index in ({}, {"manifests": []}, {"manifests": [descriptor, descriptor]},
                      {"manifests": [invalid]}, {"manifests": [non_linux]},
                      {"manifests": [None]}):
            with self.subTest(index=index), self.assertRaises(ValueError):
                security.platform_digest(index, "arm64")


if __name__ == "__main__":
    unittest.main()
