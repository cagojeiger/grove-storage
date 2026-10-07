"""Standard S3 presigned transfers against the packaged, console-enabled server."""

import hashlib
import json
import ssl
import urllib.error
import urllib.parse
import urllib.request

import boto3
from botocore.config import Config
from botocore.exceptions import ClientError

from s3_backend_fixture import docker, minio_backend


def check(endpoint, origin, password, network, report_dir):
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({}),
        urllib.request.HTTPSHandler(context=ssl._create_unverified_context()),
    )
    cookie = None

    def identity(method, path, body=None, expected=200):
        nonlocal cookie
        headers = {"Origin": origin, "X-Grove-CSRF": "1", "Content-Type": "application/json"}
        if cookie:
            headers["Cookie"] = cookie
        request = urllib.request.Request(origin + "/api/admin/identity/v1" + path,
            method=method, headers=headers,
            data=None if body is None else json.dumps(body).encode())
        with opener.open(request, timeout=10) as response:
            assert response.status == expected
            if response.headers.get("Set-Cookie"):
                cookie = response.headers["Set-Cookie"].split(";", 1)[0]
            data = response.read()
            return json.loads(data) if data else None

    identity("POST", "/session", {"username": "owner", "password": password})
    token = identity("POST", "/me/tokens", {
        "label": "image-s3-fixture", "expires_in_days": 1, "current_password": password,
    }, expected=201)["token"]

    def command(name, inputs):
        request = urllib.request.Request(endpoint + "/api/admin/commands/v1", method="POST",
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"},
            data=json.dumps({"protocol": 1, "command": name, "input": inputs}).encode())
        with opener.open(request, timeout=30) as response:
            result = json.load(response)
        assert result["protocol"] == 1 and result["command"] == name
        return result["result"]

    def rejected(request, status):
        try:
            with opener.open(request, timeout=15):
                raise AssertionError("expected rejected S3 request")
        except urllib.error.HTTPError as error:
            assert error.code == status

    with minio_backend() as backend:
        docker("network", "connect", network, backend.container)
        spec = dict(backend.spec, endpoint=f"http://{backend.container}:9000")
        command("storage.create", {"id": "image-storage", "spec": spec})
        command("client.create", {"id": "image-client", "storage_id": "image-storage"})
        credential = command("credential.create", {"client_id": "image-client"})
        identity("DELETE", "/session", expected=204)
        client = boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
            aws_access_key_id=credential["access_key_id"],
            aws_secret_access_key=credential["secret_key"],
            config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
                connect_timeout=3, read_timeout=30, retries={"total_max_attempts": 1}))
        params = {"Bucket": "image-client", "Key": "image-contract/\uD55C\uAE00 (1).bin"}
        body = b"packaged-presigned-object-proof\x00" * 4096
        put = client.generate_presigned_url("put_object", Params=params, ExpiresIn=60)
        with opener.open(urllib.request.Request(put, method="PUT", data=body), timeout=30) as response:
            assert response.status == 200
        assert client.head_object(**params)["ContentLength"] == len(body)
        get = client.generate_presigned_url("get_object", Params=params, ExpiresIn=60)
        with opener.open(get, timeout=30) as response:
            assert response.read() == body
        with opener.open(urllib.request.Request(get, headers={"Range": "bytes=0-9"}), timeout=30) as response:
            assert response.status == 206 and response.read() == body[:10]
        signature = get.split("X-Amz-Signature=", 1)[1].split("&", 1)[0]
        changed = ("0" if signature[0] != "0" else "1") + signature[1:]
        rejected(get.replace("X-Amz-Signature=" + signature, "X-Amz-Signature=" + changed), 403)
        rejected(client.generate_presigned_url("get_object", Params=params, ExpiresIn=0), 403)
        rejected(urllib.request.Request(get, headers={"Host": urllib.parse.urlsplit(origin).netloc}), 404)

        expected_etag = '"' + hashlib.md5(body).hexdigest() + '"'
        objects = backend.vendor.list_objects_v2(Bucket=spec["bucket"]).get("Contents", [])
        physical = [item for item in objects if item["ETag"] == expected_etag]
        assert len(physical) == 1
        stored = backend.vendor.get_object(Bucket=spec["bucket"], Key=physical[0]["Key"])
        try:
            assert stored["Body"].read() == body
        finally:
            stored["Body"].close()
        client.delete_object(**params)
        try:
            client.head_object(**params)
        except ClientError as error:
            assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 404
        else:
            raise AssertionError("deleted object remained readable")
        (report_dir / "s3.json").write_text(json.dumps({
            "presigned_put_get": True, "range_get": True, "invalid_signature_rejected": True,
            "expired_url_rejected": True, "console_host_isolated": True,
            "physical_bytes_verified": True, "delete_verified": True,
            "browser_session_required": False, "sha256": hashlib.sha256(body).hexdigest(),
        }, indent=2))
    print("PASS packaged S3: presigned PUT/GET, Range, signature/expiry rejection, physical bytes, delete, no browser session")
