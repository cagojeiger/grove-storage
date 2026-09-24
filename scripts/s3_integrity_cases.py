"""Request integrity and unsupported conditional writes on a disposable server."""

import base64
import hashlib
import urllib.error
import urllib.request
import zlib

from botocore.auth import S3SigV4Auth
from botocore.awsrequest import AWSRequest
from botocore.credentials import Credentials
from botocore.exceptions import ClientError


def check_integrity(client, credential, endpoint, opener):
    failures = []
    key = "integrity-contract"
    for options, expected in [({"IfNoneMatch": "*"}, "NotImplemented"),
                              ({"ContentMD5": "AAAAAAAAAAAAAAAAAAAAAA=="}, "BadDigest"),
                              ({"ChecksumCRC32": "AAAAAA=="}, "BadDigest"),
                              ({"ContentMD5": "invalid-base64"}, "InvalidDigest"),
                              ({"ChecksumSHA256": base64.b64encode(bytes(32)).decode()}, "BadDigest"),
                              ({"ChecksumSHA1": base64.b64encode(bytes(20)).decode()}, "NotImplemented")]:
        client.put_object(Bucket="s3-test", Key=key, Body=b"original")
        try:
            client.put_object(Bucket="s3-test", Key=key, Body=b"replacement", **options)
        except ClientError as error:
            assert error.response["Error"]["Code"] == expected, error.response
            assert client.get_object(Bucket="s3-test", Key=key)["Body"].read() == b"original"
            print("PASS", next(iter(options)), "rejected without replacing object")
        else:
            failures.append(f"ignored {next(iter(options))}")

    upload_id = client.create_multipart_upload(Bucket="s3-test", Key=key)["UploadId"]
    old = client.upload_part(Bucket="s3-test", Key=key, UploadId=upload_id, PartNumber=1, Body=b"original")
    signed = AWSRequest(method="PUT", data=b"original", url=endpoint + f"/s3-test/{key}?uploadId={upload_id}&partNumber=1")
    S3SigV4Auth(Credentials(credential["access_key_id"], credential["secret_key"]), "s3", "us-east-1").add_auth(signed)
    try:
        with opener.open(urllib.request.Request(signed.url, method="PUT", data=b"tampered",
                         headers=dict(signed.headers)), timeout=10) as response:
            response.read()
        failures.append("UploadPart accepted changed signed body")
    except urllib.error.HTTPError as error:
        assert error.code == 400 and b"XAmzContentSHA256Mismatch" in error.read()
        client.complete_multipart_upload(Bucket="s3-test", Key=key, UploadId=upload_id,
            MultipartUpload={"Parts": [{"PartNumber": 1, "ETag": old["ETag"]}]})
        assert client.get_object(Bucket="s3-test", Key=key)["Body"].read() == b"original"
        print("PASS signed UploadPart rejection preserves the previous part")
    for failure in failures:
        print("FAIL", failure)
    assert not failures, failures

    content = b"valid-checksums"
    client.put_object(Bucket="s3-test", Key=key, Body=content,
        ContentMD5=base64.b64encode(hashlib.md5(content).digest()).decode(),
        ChecksumCRC32=base64.b64encode(zlib.crc32(content).to_bytes(4, "big")).decode())
    assert client.get_object(Bucket="s3-test", Key=key)["Body"].read() == content
    result = client.put_object(Bucket="s3-test", Key=key, Body=content,
        ChecksumSHA256=base64.b64encode(hashlib.sha256(content).digest()).decode())
    etag = result["ETag"]
    assert client.get_object(Bucket="s3-test", Key=key, IfMatch=etag)["Body"].read() == content
    assert client.head_object(Bucket="s3-test", Key=key, IfMatch=etag)["ETag"] == etag
    client.put_object(Bucket="s3-test", Key=key, Body=b"new-version")
    for operation in [client.get_object, client.head_object]:
        try:
            operation(Bucket="s3-test", Key=key, IfMatch=etag)
        except ClientError as error:
            assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 412
        else:
            raise AssertionError("stale If-Match was accepted")
    client.delete_object(Bucket="s3-test", Key=key)
    print("PASS valid MD5/CRC32/SHA256 and GET/HEAD If-Match with stale ETag rejection")
