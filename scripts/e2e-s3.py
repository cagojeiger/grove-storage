#!/usr/bin/env python3
"""Run S3 SDK compatibility checks against disposable PostgreSQL and filesystem."""

import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import time
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parent
HARNESS = runpy.run_path(str(ROOT / "e2e-cli.py"))


def check(endpoint, directory):
    from botocore.auth import S3SigV4Auth
    from botocore.awsrequest import AWSRequest
    from botocore.credentials import Credentials
    import boto3
    from botocore.config import Config

    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def admin(method, path, body=None):
        request = urllib.request.Request(
            endpoint + path, method=method,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Authorization": "Bearer " + HARNESS["TOKEN"],
                     "Content-Type": "application/json"},
        )
        with opener.open(request, timeout=5) as response:
            return json.loads(response.read())

    deadline = time.monotonic() + 20
    while True:
        try:
            if admin("GET", "/readyz") == {"status": "ready"}:
                break
        except (urllib.error.URLError, TimeoutError):
            pass
        if time.monotonic() >= deadline:
            raise RuntimeError("server readiness timeout")
        time.sleep(0.1)

    root = Path(directory) / "s3-objects"
    root.mkdir()
    admin("POST", "/api/admin/v1/storages", {
        "id": "s3-test-fs", "kind": "fs", "root_path": str(root),
        "capacity_bytes": 1073741824,
    })
    admin("POST", "/api/admin/v1/clients", {"id": "s3-test", "storage_id": "s3-test-fs"})
    credential = admin("POST", "/api/admin/v1/clients/s3-test/s3-credentials", {})
    env = dict(os.environ, S3_ENDPOINT=endpoint, S3_BUCKET="s3-test",
               S3_ACCESS_KEY=credential["access_key_id"], S3_SECRET_KEY=credential["secret_key"],
               S3_EXPECT_WRONG_KEY_404="1", NO_PROXY="127.0.0.1")
    subprocess.run([sys.executable, str(ROOT / "s3-capture.py")], env=env, check=True, timeout=120)

    client = boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
                          aws_access_key_id=credential["access_key_id"],
                          aws_secret_access_key=credential["secret_key"],
                          config=Config(signature_version="s3v4", s3={"addressing_style": "path"}))
    upload = client.create_multipart_upload(Bucket="s3-test", Key="xml-contract")
    upload_id = upload["UploadId"]
    part = client.upload_part(Bucket="s3-test", Key="xml-contract", UploadId=upload_id,
                              PartNumber=1, Body=b"xml-body")

    def complete(body):
        url = endpoint + "/s3-test/xml-contract?uploadId=" + upload_id
        signed = AWSRequest(method="POST", url=url, data=body.encode())
        S3SigV4Auth(Credentials(credential["access_key_id"], credential["secret_key"]),
                   "s3", "us-east-1").add_auth(signed)
        return opener.open(urllib.request.Request(url, method="POST", data=body.encode(),
                                                  headers=dict(signed.headers)), timeout=10)

    fragment = f"<Part><PartNumber>1</PartNumber><ETag>{part['ETag']}</ETag>"
    try:
        complete("<CompleteMultipartUpload>" + fragment)
    except urllib.error.HTTPError as error:
        assert error.code == 400
        assert b"<Code>MalformedXML</Code>" in error.read()
    else:
        raise AssertionError("truncated XML was accepted")
    # Rejection leaves the session available for a valid completion.
    with complete('<CompleteMultipartUpload xmlns="http://s3.amazonaws.com/doc/2006-03-01/">'
                  + fragment + "</Part></CompleteMultipartUpload>") as response:
        assert response.status == 200
    assert client.get_object(Bucket="s3-test", Key="xml-contract")["Body"].read() == b"xml-body"
    url = client.generate_presigned_url("get_object", Params={"Bucket": "s3-test", "Key": "xml-contract"})
    with opener.open(url, timeout=10) as response:
        assert response.read() == b"xml-body"
    client.delete_object(Bucket="s3-test", Key="xml-contract")
    print("PASS signed malformed XML rejection, valid retry, and presigned GET")


if __name__ == "__main__":
    HARNESS["main"](check)
