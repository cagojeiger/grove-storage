"""Adversarial SigV4 cases for the disposable S3 integration server."""

import urllib.error
import urllib.request
from botocore.auth import S3SigV4Auth
from botocore.awsrequest import AWSRequest
from botocore.credentials import Credentials


def check_auth(client, credential, endpoint, opener):
    failures = []
    credentials = Credentials(credential["access_key_id"], credential["secret_key"])
    client.put_object(Bucket="s3-test", Key="auth-contract", Body=b"auth-body")

    def send(signed, data=None):
        return opener.open(urllib.request.Request(
            signed.url, method=signed.method,
            data=signed.data if data is None else data,
            headers=dict(signed.headers),
        ), timeout=10)

    def rejected(label, action, status, code):
        try:
            with action() as response:
                response.read()
                failures.append(f"{label}: accepted {response.status}")
        except urllib.error.HTTPError as error:
            body = error.read()
            if error.code != status or f"<Code>{code}</Code>".encode() not in body:
                failures.append(f"{label}: unexpected {error.code} {body!r}")
            else:
                print("PASS", label)

    url = client.generate_presigned_url("get_object", ExpiresIn=604801,
        Params={"Bucket": "s3-test", "Key": "auth-contract"})
    rejected("presigned expiry above seven days", lambda: opener.open(url, timeout=10),
             403, "AccessDenied")
    url = client.generate_presigned_url("get_object", ExpiresIn=0,
        Params={"Bucket": "s3-test", "Key": "auth-contract"})
    rejected("zero presigned expiry", lambda: opener.open(url, timeout=10), 403, "AccessDenied")
    url = client.generate_presigned_url("get_object", ExpiresIn=604800,
        Params={"Bucket": "s3-test", "Key": "auth-contract"})
    with opener.open(url, timeout=10) as response:
        assert response.read() == b"auth-body"
    print("PASS seven-day expiry boundary")

    class MissingHostSigner(S3SigV4Auth):
        def headers_to_sign(self, request):
            headers = super().headers_to_sign(request)
            del headers["host"]
            return headers

    request = AWSRequest(method="GET", url=endpoint + "/s3-test/auth-contract")
    MissingHostSigner(credentials, "s3", "us-east-1").add_auth(request)
    rejected("missing host in SignedHeaders", lambda: send(request), 403, "AccessDenied")

    request = AWSRequest(method="GET", url=endpoint + "/s3-test/auth-contract")
    S3SigV4Auth(credentials, "s3", "us-east-1").add_auth(request)
    auth = request.headers["Authorization"]
    del request.headers["Authorization"]
    request.headers["Authorization"] = auth.replace("/aws4_request,", "/aws4_request/extra,")
    rejected("extra credential scope component", lambda: send(request), 403, "AccessDenied")

    url = client.generate_presigned_url("get_object",
        Params={"Bucket": "s3-test", "Key": "auth-contract"})
    signature = url.split("X-Amz-Signature=", 1)[1].split("&", 1)[0]
    rejected("duplicate query signature", lambda: opener.open(
        url + "&X-Amz-Signature=" + signature, timeout=10), 403, "AccessDenied")

    request = AWSRequest(method="GET", url=endpoint + "/s3-test/auth-contract",
                         headers={"x-amz-meta-check": "a   b"})
    S3SigV4Auth(credentials, "s3", "us-east-1").add_auth(request)
    with send(request) as response:
        assert response.read() == b"auth-body"
    print("PASS canonical signed-header whitespace")

    upload = client.create_multipart_upload(Bucket="s3-test", Key="auth-complete")
    upload_id = upload["UploadId"]
    part = client.upload_part(Bucket="s3-test", Key="auth-complete", UploadId=upload_id,
                              PartNumber=1, Body=b"complete-body")
    body = ("<CompleteMultipartUpload><Part><PartNumber>1</PartNumber>"
            f"<ETag>{part['ETag']}</ETag></Part></CompleteMultipartUpload>").encode()
    request = AWSRequest(method="POST", data=body,
        url=endpoint + "/s3-test/auth-complete?uploadId=" + upload_id)
    S3SigV4Auth(credentials, "s3", "us-east-1").add_auth(request)
    rejected("Complete body changed after signing", lambda: send(request, body + b"\n"),
             400, "XAmzContentSHA256Mismatch")
    if not failures:
        with send(request) as response:
            assert response.status == 200
        assert client.get_object(Bucket="s3-test", Key="auth-complete")["Body"].read() == b"complete-body"
        print("PASS unchanged Complete retry after hash rejection")
        upload = client.create_multipart_upload(Bucket="s3-test", Key="auth-unsigned")
        unsigned_id = upload["UploadId"]
        part = client.upload_part(Bucket="s3-test", Key="auth-unsigned", UploadId=unsigned_id,
                                  PartNumber=1, Body=b"unsigned-body")
        unsigned_body = ("<CompleteMultipartUpload><Part><PartNumber>1</PartNumber>"
                        f"<ETag>{part['ETag']}</ETag></Part></CompleteMultipartUpload>").encode()
        url = client.generate_presigned_url("complete_multipart_upload", Params={
            "Bucket": "s3-test", "Key": "auth-unsigned", "UploadId": unsigned_id})
        with opener.open(urllib.request.Request(url, data=unsigned_body, method="POST"), timeout=10) as response:
            assert response.status == 200
        assert client.get_object(Bucket="s3-test", Key="auth-unsigned")["Body"].read() == b"unsigned-body"
        client.delete_object(Bucket="s3-test", Key="auth-unsigned")
        print("PASS presigned Complete with explicit unsigned payload semantics")
    for failure in failures:
        print("FAIL", failure)
    assert not failures, failures
    client.delete_object(Bucket="s3-test", Key="auth-contract")
    client.delete_object(Bucket="s3-test", Key="auth-complete")
