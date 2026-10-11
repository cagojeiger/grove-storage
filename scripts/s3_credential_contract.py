"""Grove-specific credential isolation and revocation at the S3 HTTP boundary."""

from s3_presigned_contract import expect, request, sdk


def check_credentials(management, endpoint, credential, bucket, opener):
    other_bucket = "isolated-client"
    management.command("client.create", {"id": other_bucket, "storage_id": "parity-storage"})
    other_key = management.command("credential.create", {"client_id": other_bucket})
    caller, other = sdk(endpoint, credential), sdk(endpoint, other_key)
    params = {"Bucket": other_bucket, "Key": "protected-object"}
    body = b"another-client-must-remain-unchanged"
    other.put_object(**params, Body=body)
    upload_id = other.create_multipart_upload(**params)["UploadId"]
    operations = [
        ("put_object", "PUT", {}, b"must-not-replace"),
        ("get_object", "GET", {}, None),
        ("head_object", "HEAD", {}, None),
        ("delete_object", "DELETE", {}, None),
        ("create_multipart_upload", "POST", {}, b""),
        ("upload_part", "PUT", {"UploadId": upload_id, "PartNumber": 1}, b"unauthorized-part"),
        ("complete_multipart_upload", "POST", {"UploadId": upload_id}, b"<CompleteMultipartUpload/>"),
        ("abort_multipart_upload", "DELETE", {"UploadId": upload_id}, None),
    ]
    for operation, method, extra, payload in operations:
        url = caller.generate_presigned_url(operation, Params={**params, **extra}, ExpiresIn=600)
        expect(request(opener, url, method, payload), 404, None if method == "HEAD" else "NoSuchBucket")
    assert other.get_object(**params)["Body"].read() == body
    part = other.upload_part(**params, UploadId=upload_id, PartNumber=1, Body=body)
    other.complete_multipart_upload(**params, UploadId=upload_id,
        MultipartUpload={"Parts": [{"PartNumber": 1, "ETag": part["ETag"]}]})
    assert other.get_object(**params)["Body"].read() == body
    other.delete_object(**params)
    print("PASS all eight S3 operations reject another Client bucket; its object/session survive")

    params = {"Bucket": bucket, "Key": "credential-revocation"}
    caller.put_object(**params, Body=body)
    upload_id = caller.create_multipart_upload(**params)["UploadId"]
    part = caller.upload_part(**params, UploadId=upload_id, PartNumber=1, Body=body)
    complete = (f"<CompleteMultipartUpload><Part><PartNumber>1</PartNumber>"
                f"<ETag>{part['ETag']}</ETag></Part></CompleteMultipartUpload>").encode()
    operations = [
        ("put_object", "PUT", {}, b"must-not-replace"),
        ("get_object", "GET", {}, None),
        ("head_object", "HEAD", {}, None),
        ("delete_object", "DELETE", {}, None),
        ("create_multipart_upload", "POST", {}, b""),
        ("upload_part", "PUT", {"UploadId": upload_id, "PartNumber": 1}, b"must-not-replace"),
        ("complete_multipart_upload", "POST", {"UploadId": upload_id}, complete),
        ("abort_multipart_upload", "DELETE", {"UploadId": upload_id}, None),
    ]
    issued = [(method, payload, caller.generate_presigned_url(operation,
               Params={**params, **extra}, ExpiresIn=600)) for operation, method, extra, payload in operations]
    replacement_key = management.command("credential.create", {"client_id": bucket})
    replacement = sdk(endpoint, replacement_key)
    management.command("credential.delete", {"client_id": bucket, "access_key_id": credential["access_key_id"]})
    for method, payload, url in issued:
        expect(request(opener, url, method, payload), 403, None if method == "HEAD" else "InvalidAccessKeyId")
    assert replacement.get_object(**params)["Body"].read() == body
    replacement.complete_multipart_upload(**params, UploadId=upload_id,
        MultipartUpload={"Parts": [{"PartNumber": 1, "ETag": part["ETag"]}]})
    assert replacement.get_object(**params)["Body"].read() == body

    for name, key, code in [
        ("unknown_key", {"access_key_id": "nonexistent-fixture-key", "secret_key": "unknown"}, "InvalidAccessKeyId"),
        ("wrong_secret", {**replacement_key, "secret_key": "wrong-fixture-secret"}, "SignatureDoesNotMatch"),
    ]:
        url = sdk(endpoint, key).generate_presigned_url("get_object", Params=params, ExpiresIn=600)
        expect(request(opener, url), 403, code)
        print("PASS", name, "HTTP status and XML code")
    url = replacement.generate_presigned_url("put_object", Params={**params, "IfNoneMatch": "*"}, ExpiresIn=600)
    expect(request(opener, url, "PUT", body), 403, "AccessDenied")
    assert replacement.get_object(**params)["Body"].read() == body
    replacement.delete_object(**params)
    print("PASS revoked presigned URLs reject all eight operations; replacement credential preserves access")
    return {"cross_client_operations": len(operations), "revoked_operations": len(issued),
            "replacement_key_works": True, "unknown_key": True, "wrong_secret": True,
            "missing_signed_header": True, "original_object_and_multipart_preserved": True}
