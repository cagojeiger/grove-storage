"""Completion validation against the disposable S3 server."""

from botocore.exceptions import ClientError


def check_multipart(client):
    failures = []
    for case, expected in [("small", "EntityTooSmall"), ("order", "InvalidPartOrder"),
                           ("duplicate", "InvalidPartOrder"), ("missing", "InvalidPart"),
                           ("etag", "InvalidPart")]:
        key = "multipart-contract-" + case
        client.put_object(Bucket="s3-test", Key=key, Body=b"old-object")
        upload_id = client.create_multipart_upload(Bucket="s3-test", Key=key)["UploadId"]
        parts = []
        for number in [1, 2]:
            result = client.upload_part(Bucket="s3-test", Key=key, UploadId=upload_id,
                                        PartNumber=number, Body=b"small")
            parts.append({"PartNumber": number, "ETag": result["ETag"]})
        listed = [dict(part) for part in parts]
        if case == "order":
            listed.reverse()
        elif case == "duplicate":
            listed = [parts[0], parts[0]]
        elif case == "missing":
            listed[1]["PartNumber"] = 3
        elif case == "etag":
            listed[1]["ETag"] = '"' + "0" * 32 + '"'
        try:
            client.complete_multipart_upload(Bucket="s3-test", Key=key, UploadId=upload_id,
                                              MultipartUpload={"Parts": listed})
        except ClientError as error:
            code = error.response["Error"]["Code"]
            if code != expected or error.response["ResponseMetadata"]["HTTPStatusCode"] != 400:
                failures.append(f"{case}: expected {expected}, got {code}")
                continue
        else:
            failures.append(f"{case}: completion unexpectedly succeeded")
            continue
        assert client.get_object(Bucket="s3-test", Key=key)["Body"].read() == b"old-object"
        # Repair the same upload; the exact minimum is valid for the first part.
        content = b"x" * (5 * 1024 * 1024)
        result = client.upload_part(Bucket="s3-test", Key=key, UploadId=upload_id,
                                    PartNumber=1, Body=content)
        parts[0]["ETag"] = result["ETag"]
        client.complete_multipart_upload(Bucket="s3-test", Key=key, UploadId=upload_id,
                                          MultipartUpload={"Parts": parts})
        assert client.get_object(Bucket="s3-test", Key=key)["Body"].read() == content + b"small"
        client.delete_object(Bucket="s3-test", Key=key)
        print("PASS", expected, "preserves old object and allows repaired completion")
    for failure in failures:
        print("FAIL", failure)
    assert not failures, failures
