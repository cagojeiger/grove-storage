import sys
import subprocess
from pathlib import Path
import unittest
from unittest.mock import MagicMock, patch

from botocore.exceptions import ClientError

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from s3_backend_fixture import MinioBackend, build_minio_image, minio_backend


class MinioBuildTests(unittest.TestCase):
    def setUp(self):
        build_minio_image.cache_clear()
        self.addCleanup(build_minio_image.cache_clear)

    @patch("s3_backend_fixture.subprocess.run")
    def test_builds_once_per_process(self, run):
        build_minio_image()
        build_minio_image()
        run.assert_called_once()
        self.assertEqual(run.call_args.args[0][:3], ["docker", "build", "--tag"])
        self.assertTrue(run.call_args.kwargs["check"])

    @patch("s3_backend_fixture.docker")
    @patch("s3_backend_fixture.subprocess.run")
    def test_build_failure_stops_before_container_creation(self, run, docker):
        run.side_effect = subprocess.CalledProcessError(1, ["docker", "build"])
        with self.assertRaises(subprocess.CalledProcessError):
            with minio_backend():
                self.fail("failed build must not yield a backend")
        docker.assert_not_called()
        run.assert_called_once()
        run.side_effect = None
        build_minio_image()
        self.assertEqual(run.call_count, 2)


class MinioReadinessTests(unittest.TestCase):
    def setUp(self):
        self.backend = MinioBackend.__new__(MinioBackend)
        self.backend.endpoint = "http://127.0.0.1:9000"
        self.backend.vendor = MagicMock()
        opener = MagicMock()
        opener.open.return_value.__enter__.return_value.status = 200
        self.opener = patch("s3_backend_fixture.urllib.request.build_opener", return_value=opener)
        self.opener.start()
        self.addCleanup(self.opener.stop)

    @patch("s3_backend_fixture.time.sleep")
    def test_waits_for_s3_initialization_after_health_success(self, sleep):
        self.backend.vendor.list_buckets.side_effect = [
            ClientError({"Error": {"Code": "XMinioServerNotInitialized"}}, "ListBuckets"),
            {},
        ]
        self.backend.ready()
        self.assertEqual(self.backend.vendor.list_buckets.call_count, 2)
        sleep.assert_called_once_with(0.2)

    def test_does_not_hide_authentication_failure(self):
        self.backend.vendor.list_buckets.side_effect = ClientError(
            {"Error": {"Code": "AccessDenied"}}, "ListBuckets"
        )
        with self.assertRaises(ClientError):
            self.backend.ready()

    @patch("s3_backend_fixture.time.sleep")
    @patch("s3_backend_fixture.time.monotonic", side_effect=[0, 0, 31])
    def test_initialization_wait_is_bounded(self, monotonic, sleep):
        self.backend.vendor.list_buckets.side_effect = ClientError(
            {"Error": {"Code": "XMinioServerNotInitialized"}}, "ListBuckets"
        )
        with self.assertRaisesRegex(RuntimeError, "readiness timeout"):
            self.backend.ready()


if __name__ == "__main__":
    unittest.main()
