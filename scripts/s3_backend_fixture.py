"""Disposable MinIO backend; never connects to a configured production endpoint."""

from contextlib import contextmanager
import hashlib
import json
import socket
import subprocess
import time
import urllib.request
import uuid

import boto3
from botocore.config import Config
from botocore.exceptions import ClientError


IMAGE = "quay.io/minio/minio:RELEASE.2025-09-07T16-13-09Z"


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True, timeout=120).strip()


class MinioBackend:
    def __init__(self, container, endpoint, secret):
        self.container = container
        self.endpoint = endpoint
        self.spec = {
            "kind": "s3", "endpoint": endpoint, "region": "us-east-1",
            "bucket": "grove-contract", "force_path_style": True,
            "access_key": "grove-contract", "secret_key": secret,
            "capacity_bytes": 1073741824,
        }
        self.vendor = boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
            aws_access_key_id=self.spec["access_key"], aws_secret_access_key=secret,
            config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
                          connect_timeout=3, read_timeout=10, retries={"total_max_attempts": 1}))

    def ready(self):
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                with opener.open(self.endpoint + "/minio/health/ready", timeout=2) as response:
                    if response.status == 200:
                        self.vendor.list_buckets()
                        return
            except ClientError as error:
                if error.response["Error"]["Code"] != "XMinioServerNotInitialized":
                    raise
            except OSError:
                pass
            time.sleep(0.2)
        raise RuntimeError("disposable MinIO readiness timeout")

    def verify(self, client, credential):
        body = b"physical-vendor-proof-" + uuid.uuid4().hex.encode()
        client.put_object(Bucket="s3-test", Key="backend-proof", Body=body)
        expected = '"' + hashlib.md5(body).hexdigest() + '"'
        objects = self.vendor.list_objects_v2(Bucket=self.spec["bucket"]).get("Contents", [])
        matches = [obj for obj in objects if obj["ETag"] == expected]
        assert len(matches) == 1, "expected uploaded object in the vendor bucket"
        physical = self.vendor.get_object(Bucket=self.spec["bucket"], Key=matches[0]["Key"])
        assert physical["Body"].read() == body
        assert not self.vendor.list_multipart_uploads(Bucket=self.spec["bucket"]).get("Uploads", [])
        print("PASS vendor object bytes and no unfinished multipart sessions")

        # Stop only the container this fixture created; use a no-retry caller.
        caller = boto3.client("s3", endpoint_url=client.meta.endpoint_url,
            aws_access_key_id=credential["access_key_id"],
            aws_secret_access_key=credential["secret_key"],
            region_name="us-east-1", config=Config(signature_version="s3v4",
                s3={"addressing_style": "path"}, connect_timeout=3, read_timeout=30,
                retries={"total_max_attempts": 1}))
        docker("stop", "--time", "5", self.container)
        try:
            try:
                caller.get_object(Bucket="s3-test", Key="backend-proof")
            except ClientError as error:
                assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 503
                assert error.response["Error"]["Code"] == "ServiceUnavailable"
            else:
                raise AssertionError("vendor outage was not surfaced as 503")
        finally:
            docker("start", self.container)
            self.ready()
        assert caller.get_object(Bucket="s3-test", Key="backend-proof")["Body"].read() == body
        client.delete_object(Bucket="s3-test", Key="backend-proof")
        print("PASS vendor stop returns 503; restart preserves readable object")


@contextmanager
def minio_backend():
    name = "grove-contract-minio-" + uuid.uuid4().hex[:12]
    secret = uuid.uuid4().hex
    # An explicitly published port survives stop/start; Docker's dynamic
    # publication can choose a new host port on restart.
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    try:
        docker("run", "-d", "--name", name, "-p", f"127.0.0.1:{port}:9000",
               "-e", "MINIO_ROOT_USER=grove-contract", "-e", "MINIO_ROOT_PASSWORD=" + secret,
               IMAGE, "server", "/data")
        backend = MinioBackend(name, f"http://127.0.0.1:{port}", secret)
        backend.ready()
        backend.vendor.create_bucket(Bucket=backend.spec["bucket"])
        identity = json.loads(docker("image", "inspect", IMAGE))[0]["Id"]
        print("MinIO image:", IMAGE, identity)
        yield backend
    finally:
        subprocess.run(["docker", "rm", "-f", "-v", name], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
        print("PASS disposable MinIO container and volumes cleaned up")
