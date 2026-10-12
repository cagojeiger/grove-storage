from datetime import datetime, timezone
from pathlib import Path
import runpy
import unittest
from unittest.mock import Mock, patch

verification = runpy.run_path(str(Path(__file__).resolve().parents[1] / "verify-filegate-migration.py"))


class MigrationReadTests(unittest.TestCase):
    def client(self, size=3):
        client = Mock()
        client.head_object.return_value = {
            "ContentLength": size, "ETag": '"etag"', "ContentType": "text/plain",
        }
        client.generate_presigned_url.return_value = "https://s3.test/signed"
        return client

    def response(self, body, status=206):
        response = Mock()
        response.status, response.read.return_value = status, body
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)
        return response

    def test_compares_presigned_range_bytes_with_optional_headers_absent(self):
        source, target = self.client(), self.client()
        with patch("urllib.request.urlopen", side_effect=[self.response(b"abc"), self.response(b"abc")]):
            verification["probe"](source, target, "consumer", ("a%2Fb space", (3, "etag", "text/plain")))
        params = target.generate_presigned_url.call_args.kwargs["Params"]
        self.assertEqual(params, {"Bucket": "consumer", "Key": "a%2Fb space", "Range": "bytes=0-2"})

    def test_rejects_wrong_bytes_and_mismatched_head_without_printing_keys(self):
        source, target = self.client(), self.client()
        with patch("urllib.request.urlopen", side_effect=[self.response(b"abc"), self.response(b"abd")]):
            with self.assertRaisesRegex(RuntimeError, "bytes differ"):
                verification["probe"](source, target, "consumer", ("private-name", (3, "etag", "text/plain")))
        target.head_object.return_value["ContentLength"] = 4
        with patch("urllib.request.urlopen") as request:
            with self.assertRaisesRegex(RuntimeError, "HEAD size differs"):
                verification["probe"](source, target, "consumer", ("private-name", (3, "etag", "text/plain")))
            request.assert_not_called()

    def test_zero_byte_object_and_target_only_post_cutover_probe(self):
        target = self.client(size=0)
        with patch("urllib.request.urlopen", return_value=self.response(b"", 200)):
            verification["probe"](None, target, "consumer", ("zero", (0, "etag", "text/plain")))
        self.assertNotIn("Range", target.generate_presigned_url.call_args.kwargs["Params"])

    def test_optional_timestamp_presence_is_compared_when_source_is_available(self):
        source, target = self.client(), self.client()
        source.head_object.return_value["LastModified"] = datetime(2026, 1, 1, tzinfo=timezone.utc)
        with self.assertRaisesRegex(RuntimeError, "timestamp differs"):
            verification["probe"](source, target, "consumer", ("key", (3, "etag", "text/plain")))

    def test_snapshot_client_selection_preserves_literal_logical_keys(self):
        snapshot = {"tables": {
            "files": [{"id": "f", "declared_size": 3, "etag": "etag", "content_type": None}],
            "s3_keys": [{"client_id": "consumer", "key": "a%2Fb space", "file_id": "f"},
                        {"client_id": "other", "key": "hidden", "file_id": "f"}],
        }}
        self.assertEqual(verification["expected"](snapshot, "consumer"),
                         {"a%2Fb space": (3, "etag", "application/octet-stream")})


if __name__ == "__main__":
    unittest.main()
