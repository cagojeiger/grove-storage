"""Prepare standard SDK URLs for the same browser fetch contract on both endpoints."""

import hashlib
import json
from pathlib import Path
import subprocess

from botocore.exceptions import ClientError

from s3_presigned_contract import MIB, changed_query

ROOT = Path(__file__).resolve().parent


def check_browser(origin, targets, report_dir):
    fixtures = []
    for name, client, bucket in targets:
        def signed(operation, key, **params):
            return client.generate_presigned_url(operation,
                Params={"Bucket": bucket, "Key": key, **params}, ExpiresIn=600)

        single, multi, blocked = "browser/\uD55C\uAE00 +%.bin", "browser/multipart", "browser/cors-blocked"
        body = "browser-presigned-conditional-object"
        upload_id = client.create_multipart_upload(Bucket=bucket, Key=multi)["UploadId"]
        abort_id = client.create_multipart_upload(Bucket=bucket, Key="browser/abort")["UploadId"]
        get = signed("get_object", single, ResponseContentType="application/octet-stream")
        fixtures.append({"name": name, "body": body,
            "put": signed("put_object", single, ContentType="application/octet-stream",
                          ContentLength=len(body), IfNoneMatch="*"),
            "get": get, "head": signed("head_object", single), "delete": signed("delete_object", single),
            "invalid": changed_query(get, "response-content-type", "text/plain"),
            "blocked": signed("put_object", blocked, ContentType="application/octet-stream"),
            "parts": [signed("upload_part", multi, UploadId=upload_id, PartNumber=number) for number in (1, 2)],
            "complete": signed("complete_multipart_upload", multi, UploadId=upload_id),
            "multipart_get": signed("get_object", multi), "multipart_delete": signed("delete_object", multi),
            "abort": signed("abort_multipart_upload", "browser/abort", UploadId=abort_id),
            "aborted_part": signed("upload_part", "browser/abort", UploadId=abort_id, PartNumber=1)})
    completed = subprocess.run(["node", str(ROOT / "s3_browser_contract.mjs")],
        cwd=ROOT.parent / "frontend/web", input=json.dumps({"origin": origin, "targets": fixtures}),
        capture_output=True, text=True, timeout=180)
    if completed.returncode:
        raise AssertionError("browser presigned contract failed: " + completed.stderr[-2000:])
    results = json.loads(completed.stdout)
    assert set(results) == {name for name, *_ in targets}
    for name, client, bucket in targets:
        assert results[name]["multipart_sha256"] == hashlib.sha256(b"m" * (5 * MIB) + b"last-part").hexdigest()
        try:
            client.head_object(Bucket=bucket, Key="browser/cors-blocked")
        except ClientError as error:
            assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 404
        else:
            raise AssertionError("disallowed Origin wrote an object")
    assert results["minio"] == results["grove"], results
    (report_dir / "browser.json").write_text(json.dumps(results, indent=2))
    print("PASS identical Chromium HTTPS PUT/GET/HEAD/DELETE, multipart, readable XML errors and blocked CORS writes")
    return results
