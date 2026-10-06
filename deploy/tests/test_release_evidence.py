import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release_evidence", ROOT / "deploy/ci/release-evidence.py")
evidence = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evidence)


class ReleaseEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.env = {
            "RELEASE_SHA": "a" * 40, "GITHUB_WORKFLOW_SHA": "b" * 40,
            "GITHUB_SERVER_URL": "https://github.com", "GITHUB_REPOSITORY": "cagojeiger/grove-storage",
            "GITHUB_EVENT_NAME": "workflow_run", "RUNNER_ENVIRONMENT": "github-hosted",
            "CI_RUN_ID": "123", "CI_RUN_ATTEMPT": "1",
            "GITHUB_WORKFLOW_REF": "cagojeiger/grove-storage/.github/workflows/release.yml@refs/heads/main",
            "GITHUB_RUN_ID": "456", "GITHUB_RUN_ATTEMPT": "1",
        }
        self.expected = evidence.predicate(self.env)

    def verified(self, predicate_type="predicateType"):
        return [{"verificationResult": {"statement": {
            predicate_type: evidence.PREDICATE_TYPE, "predicate": copy.deepcopy(self.expected),
        }}}]

    def test_source_and_workflow_shas_remain_distinct(self):
        self.assertNotEqual(self.expected["source"]["commit"], self.expected["workflow"]["commit"])

    def test_invalid_source_and_workflow_shas_fail(self):
        for key in ("RELEASE_SHA", "GITHUB_WORKFLOW_SHA"):
            env = dict(self.env, **{key: "main"})
            with self.subTest(key=key), self.assertRaises(ValueError):
                evidence.predicate(env)

    def test_untrusted_event_and_runner_fail(self):
        for key, value in (("GITHUB_EVENT_NAME", "pull_request"),
                           ("RUNNER_ENVIRONMENT", "self-hosted")):
            with self.subTest(key=key), self.assertRaises(ValueError):
                evidence.predicate(dict(self.env, **{key: value}))

    def test_both_gh_verified_field_spellings_pass(self):
        for field in ("predicateType", "predicate_type"):
            evidence.verify(self.verified(field), self.expected)

    def test_missing_empty_or_raw_unverified_bundle_fails(self):
        for results in (None, [], [{"attestation": {"statement": self.expected}}]):
            with self.subTest(results=results), self.assertRaises(ValueError):
                evidence.verify(results, self.expected)

    def test_source_workflow_ci_and_invocation_mismatch_fail(self):
        for section, field in (("source", "commit"), ("workflow", "commit"),
                               ("ci", "runId"), ("ci", "runAttempt"),
                               ("invocation", "runId"), ("invocation", "runAttempt")):
            results = self.verified()
            results[0]["verificationResult"]["statement"]["predicate"][section][field] = "other"
            with self.subTest(section=section, field=field), self.assertRaises(ValueError):
                evidence.verify(results, self.expected)

    def test_wrong_and_conflicting_predicate_types_fail(self):
        results = self.verified()
        statement = results[0]["verificationResult"]["statement"]
        statement["predicate_type"] = "https://slsa.dev/provenance/v1"
        with self.assertRaises(ValueError):
            evidence.verify(results, self.expected)
        del statement["predicateType"]
        with self.assertRaises(ValueError):
            evidence.verify(results, self.expected)


if __name__ == "__main__":
    unittest.main()
