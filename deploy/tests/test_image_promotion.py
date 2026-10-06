import importlib.util
import io
import json
from pathlib import Path
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("promote_image", ROOT / "deploy/ci/promote-image.py")
promotion = importlib.util.module_from_spec(spec)
spec.loader.exec_module(promotion)


class RegistryResponse(io.BytesIO):
    def __init__(self, body=b"", headers=None):
        super().__init__(body)
        self.headers = headers or {}


class ImagePromotionTests(unittest.TestCase):
    image = "ghcr.io/cagojeiger/grove-storage"
    digest = "sha256:" + "a" * 64

    def test_existing_version_is_never_overwritten(self):
        with patch.object(promotion, "version_digest", return_value=self.digest), \
                patch.object(promotion.subprocess, "run") as run, self.assertRaises(ValueError):
            promotion.promote(self.image, "1.2.3", self.digest)
        run.assert_not_called()

    def test_registry_error_never_means_missing_version(self):
        for error in (URLError("offline"), HTTPError("registry", 401, "unauthorized", {}, None),
                      HTTPError("registry", 503, "unavailable", {}, None)):
            with self.subTest(error=error), \
                    patch.object(promotion, "version_digest", side_effect=error), \
                    patch.object(promotion.subprocess, "run") as run, self.assertRaises(type(error)):
                promotion.promote(self.image, "1.2.3", self.digest)
            run.assert_not_called()

    def test_verified_digest_is_the_only_promotion_source(self):
        with patch.object(promotion, "version_digest", side_effect=[None, self.digest, self.digest]), \
                patch.object(promotion.subprocess, "run") as run:
            promotion.promote(self.image, "1.2.3", self.digest)
        self.assertEqual(run.call_args.args[0][-1], f"{self.image}@{self.digest}")
        self.assertTrue(run.call_args.kwargs["check"])

    def test_wrong_published_digest_fails(self):
        with patch.object(promotion, "version_digest", side_effect=[None, "sha256:" + "b" * 64]), \
                patch.object(promotion.subprocess, "run"), self.assertRaises(ValueError):
            promotion.promote(self.image, "1.2.3", self.digest)

    def test_invalid_version_or_digest_cannot_reach_registry(self):
        for version, digest in (("latest", self.digest), ("1.2.3", "latest")):
            with self.subTest(version=version, digest=digest), \
                    patch.object(promotion, "version_digest") as lookup, self.assertRaises(ValueError):
                promotion.promote(self.image, version, digest)
            lookup.assert_not_called()

    def test_only_manifest_404_means_missing(self):
        token = RegistryResponse(json.dumps({"token": "test-token"}).encode())
        not_found = HTTPError("registry", 404, "missing", {}, None)
        with patch.dict(promotion.os.environ, {"GITHUB_ACTOR": "test", "GH_TOKEN": "test"}), \
                patch.object(promotion, "urlopen", side_effect=[token, not_found]):
            self.assertIsNone(promotion.version_digest(self.image, "1.2.3"))

    def test_token_404_is_not_a_missing_manifest(self):
        with patch.dict(promotion.os.environ, {"GITHUB_ACTOR": "test", "GH_TOKEN": "test"}), \
                patch.object(promotion, "urlopen", side_effect=HTTPError("token", 404, "missing", {}, None)), \
                self.assertRaises(HTTPError):
            promotion.version_digest(self.image, "1.2.3")

    def test_successful_head_requires_valid_digest(self):
        for digest in (None, "latest", self.digest):
            responses = [RegistryResponse(b'{"token":"test"}'),
                         RegistryResponse(headers={"Docker-Content-Digest": digest})]
            with self.subTest(digest=digest), \
                    patch.dict(promotion.os.environ, {"GITHUB_ACTOR": "test", "GH_TOKEN": "test"}), \
                    patch.object(promotion, "urlopen", side_effect=responses):
                if digest == self.digest:
                    self.assertEqual(promotion.version_digest(self.image, "1.2.3"), digest)
                else:
                    with self.assertRaises((ValueError, TypeError)):
                        promotion.version_digest(self.image, "1.2.3")


if __name__ == "__main__":
    unittest.main()
