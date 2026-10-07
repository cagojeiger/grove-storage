from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ImageWorkflowTests(unittest.TestCase):
    def test_external_actions_use_full_commit_shas(self):
        for path in (ROOT / ".github").rglob("*.yml"):
            for action in re.findall(r"uses:\s*([^\s#]+)", path.read_text()):
                if action.startswith("./"):
                    continue
                with self.subTest(path=path, action=action):
                    self.assertRegex(action, r"^[\w/-]+@[a-f0-9]{40}$")

    def test_base_images_and_rust_inventory_are_fixed(self):
        dockerfile = (ROOT / "deploy/docker/Dockerfile").read_text()
        for image in re.findall(r"^FROM\s+(\S+)", dockerfile, re.MULTILINE):
            if image in {"chef"}:
                continue
            self.assertRegex(image, r"@sha256:[a-f0-9]{64}$")
        self.assertIn("cargo-auditable --version 0.7.6 --locked", dockerfile)
        self.assertIn("cargo auditable build --release --locked --bin grove-storage", dockerfile)

    def test_runtime_preserves_non_root_identity_without_package_installation(self):
        dockerfile = (ROOT / "deploy/docker/Dockerfile").read_text()
        runtime = dockerfile[dockerfile.index("FROM gcr.io/distroless/"):]
        self.assertIn("cc-debian13:nonroot@sha256:", runtime)
        self.assertIn("USER 10001:10001", runtime)
        self.assertIn('ENTRYPOINT ["/usr/local/bin/grove-storage"]', runtime)
        self.assertNotRegex(runtime, r"(?m)^RUN\s")

    def test_console_is_built_locked_and_shipped_without_node_runtime(self):
        dockerfile = (ROOT / "deploy/docker/Dockerfile").read_text()
        self.assertIn("npm ci && npm audit --audit-level=high", dockerfile)
        self.assertIn("RUN npm run build", dockerfile)
        runtime = dockerfile[dockerfile.index("FROM gcr.io/distroless/"):]
        self.assertIn("/app/frontend/web/dist /app/web", runtime)
        self.assertIn("/app/inventory/frontend/package-lock.json", runtime)
        self.assertIn("GROVE_CONSOLE_DIST_DIR=/app/web", runtime)
        self.assertNotIn("node_modules", runtime)
        for path in ("ci.yml", "release.yml"):
            self.assertIn("scripts/e2e-image.py", (ROOT / ".github/workflows" / path).read_text())

    def test_image_runtime_gate_installs_pinned_s3_sdk_and_checks_objects(self):
        for path in ("ci.yml", "release.yml"):
            workflow = (ROOT / ".github/workflows" / path).read_text()
            self.assertIn("/tmp/grove-image-sdk/bin/pip install boto3==1.43.99", workflow)
            self.assertIn("/tmp/grove-image-sdk/bin/python -B scripts/e2e-image.py", workflow)
        fixture = (ROOT / "scripts/e2e-image.py").read_text()
        self.assertIn("check_s3(endpoint, origin, password, name, report_dir)", fixture)

    def test_image_database_readiness_requires_tcp(self):
        fixture = (ROOT / "scripts/e2e-image.py").read_text()
        self.assertIn('"pg_isready", "-h", "127.0.0.1", "-U", "grove", "-d", "grove"', fixture)

    def test_image_policy_cannot_ignore_unfixed_findings_or_rust_inventory(self):
        action = (ROOT / ".github/actions/image-security/action.yml").read_text()
        self.assertIn("ignore-unfixed: false", action)
        self.assertIn("TRIVY_IGNOREFILE: /dev/null", action)
        self.assertIn("TRIVY_LIST_ALL_PKGS: 'true'", action)
        self.assertIn("scanners: vuln,secret", action)
        self.assertIn("python3 deploy/ci/image-security.py validate", action)
        self.assertLess(action.index("Preserve scan evidence"), action.index("Enforce image security policy"))

    def test_frontend_scan_uses_the_image_inventory_not_checkout_files(self):
        action = (ROOT / ".github/actions/image-security/action.yml").read_text()
        self.assertIn('docker create --platform "linux/$ARCHITECTURE" "$IMAGE"', action)
        self.assertIn('docker cp "$container:/app/inventory/frontend/package-lock.json"', action)
        self.assertIn("trivy fs --scanners vuln --pkg-types library", action)
        self.assertIn('trap \'docker rm "$container" >/dev/null\' EXIT', action)
        self.assertIn("'.Results += $frontend[0].Results'", action)
        self.assertNotIn("frontend/web/package-lock.json", action)

    def test_release_checks_candidates_and_final_index_before_promotion(self):
        release = (ROOT / ".github/workflows/release.yml").read_text()
        steps = ["Scan candidate before signing", "Test packaged release console and API",
                 "Sign scanned candidate evidence",
                 "Verify signed candidate evidence", "Export digest", "Sign final index",
                 "Verify final index before promotion", "Promote verified digest"]
        positions = [release.index(step) for step in steps]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("sbom: true", release)
        self.assertIn("provenance: mode=max", release)
        self.assertIn("github.event.workflow_run.event == 'push'", release)
        self.assertIn("github.event.workflow_run.head_repository.full_name == github.repository", release)
        self.assertIn("--deny-self-hosted-runners", release)

    def test_ci_scans_both_architectures_without_registry_write_permissions(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        job = ci[ci.index("  image-security:"):ci.index("  frontend:")]
        self.assertIn("architecture: amd64", job)
        self.assertIn("architecture: arm64", job)
        self.assertIn("load: true", job)
        self.assertIn("source: docker", job)
        self.assertNotIn("packages: write", job)

    def test_rescan_pins_index_before_resolving_platforms(self):
        workflow = (ROOT / ".github/workflows/security-rescan.yml").read_text()
        self.assertIn('"$IMAGE@$index_digest" --raw', workflow)
        self.assertIn("for architecture in amd64 arm64", workflow)
        self.assertIn("./.github/actions/image-security", workflow)


if __name__ == "__main__":
    unittest.main()
