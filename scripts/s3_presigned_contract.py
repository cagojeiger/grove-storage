"""Endpoint-independent S3 presigned contract; results contain no credentials or URLs."""

from datetime import datetime, timedelta, timezone
import hashlib
import urllib.error
import urllib.parse
import urllib.request
from unittest.mock import patch
import xml.etree.ElementTree as ET

import boto3
from botocore.config import Config

MIB = 1024 * 1024


def sdk(endpoint, credential, verify=True):
    return boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
        aws_access_key_id=credential["access_key_id"],
        aws_secret_access_key=credential["secret_key"], verify=verify,
        config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
            connect_timeout=3, read_timeout=30, retries={"total_max_attempts": 1}))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def request(opener, url, method="GET", body=None, headers=None):
    req = urllib.request.Request(url, method=method, data=body, headers=headers or {})
    try:
        response = opener.open(req, timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        return response.status, {k.lower(): v for k, v in response.headers.items()}, response.read()


def xml_value(body, name):
    return next((node.text for node in ET.fromstring(body).iter()
                 if node.tag.rsplit("}", 1)[-1] == name), None)


def observe(result, *, headers=()):
    status, fields, body = result
    return {"status": status,
            "error_code": xml_value(body, "Code") if status >= 400 and body else None,
            "headers": {name: fields.get(name) for name in headers},
            "sha256": hashlib.sha256(body).hexdigest() if status < 400 else None}


def expect(result, status, code=None):
    assert result[0] == status, (result[0], status)
    if code:
        assert xml_value(result[2], "Code") == code, (result[0], result[2][:300])


def changed_query(url, name, value):
    parsed = urllib.parse.urlsplit(url)
    query = [(key, value if key == name else old)
             for key, old in urllib.parse.parse_qsl(parsed.query, keep_blank_values=True)]
    return urllib.parse.urlunsplit(parsed._replace(query=urllib.parse.urlencode(query)))


def run_contract(client, bucket, opener):
    cases, differences = {}, {}

    def signed(operation, key, **params):
        return client.generate_presigned_url(operation,
            Params={"Bucket": bucket, "Key": key, **params}, ExpiresIn=600)

    def record(name, result, status, code=None, headers=()):
        cases[name] = observe(result, headers=headers)
        try:
            expect(result, status, code)
        except AssertionError as error:
            raise AssertionError({"case": name, "observed": cases[name], "expected_status": status}) from error
        return result

    keys = ["contract/empty", "contract/a b", "contract/a+b", "contract/a%20b",
            "contract/a%2Fb", "contract/a/b", "contract/\uD55C\uAE00 (1).bin"]
    bodies = [b"" if index == 0 else f"distinct-object-{index}".encode()
              for index in range(len(keys))]
    for index, (key, body) in enumerate(zip(keys, bodies)):
        put = signed("put_object", key, ContentType="application/octet-stream", ContentLength=len(body))
        result = record(f"key_{index}_put", request(opener, put, "PUT", body,
            {"Content-Type": "application/octet-stream"}), 200, headers=("etag",))
        assert result[1]["etag"] == '"' + hashlib.md5(body).hexdigest() + '"'
    # Read all keys only after writing them all, so encoding aliases cannot pass.
    for index, (key, body) in enumerate(zip(keys, bodies)):
        get = signed("get_object", key)
        result = record(f"key_{index}_get", request(opener, get), 200,
                        headers=("etag", "content-length", "content-type"))
        assert result[2] == body
        result = record(f"key_{index}_head", request(opener, signed("head_object", key), "HEAD"),
                        200, headers=("etag", "content-length", "content-type"))
        assert result[1]["content-length"] == str(len(body)) and not result[2]

    key, body = keys[-1], bodies[-1]
    get = signed("get_object", key, ResponseContentType="audio/webm",
                 ResponseContentDisposition="inline; filename*=UTF-8''test%20clip.webm",
                 ResponseCacheControl="private, no-store")
    result = record("response_overrides", request(opener, get), 200,
                    headers=("content-type", "content-disposition", "cache-control"))
    assert result[1]["content-type"] == "audio/webm"
    assert result[1]["content-disposition"] == "inline; filename*=UTF-8''test%20clip.webm"
    assert result[1]["cache-control"] == "private, no-store"
    for name, span, expected, start in [
        ("bounded", "bytes=0-9", body[:10], 0),
        ("open", "bytes=7-", body[7:], 7),
        ("clamped", "bytes=7-999", body[7:], 7),
        ("last", f"bytes={len(body)-1}-", body[-1:], len(body)-1),
    ]:
        result = record("range_" + name, request(opener, get, headers={"Range": span}), 206,
                        headers=("content-range", "content-length", "content-type"))
        assert result[2] == expected
        assert result[1]["content-range"] == f"bytes {start}-{start+len(expected)-1}/{len(body)}"
    for name, url, span, total in [("range_past_end", get, f"bytes={len(body)}-", len(body)),
                                   ("range_empty", signed("get_object", keys[0]), "bytes=0-", 0)]:
        result = record(name, request(opener, url, headers={"Range": span}), 416, "InvalidRange")
        differences[name + "_headers"] = observe(result, headers=("content-range",))
        assert result[1].get("content-range") in (None, f"bytes */{total}")
    result = request(opener, get, headers={"Range": "bytes=-5"})
    differences["suffix_range"] = observe(result, headers=("content-range", "content-length"))
    assert result[2] == (body[-5:] if result[0] == 206 else body)
    result = request(opener, signed("put_object", "contract/a//b"), "PUT", b"double-slash")
    differences["double_slash_key"] = observe(result)
    if result[0] == 200:
        assert request(opener, signed("get_object", "contract/a//b"))[2] == b"double-slash"
        assert request(opener, signed("get_object", keys[-2]))[2] == bodies[-2]
        expect(request(opener, signed("delete_object", "contract/a//b"), "DELETE"), 204)

    signature = dict(urllib.parse.parse_qsl(urllib.parse.urlsplit(get).query))["X-Amz-Signature"]
    bad_signature = ("0" if signature[0] != "0" else "1") + signature[1:]
    for name, url, method in [
        ("signature", changed_query(get, "X-Amz-Signature", bad_signature), "GET"),
        ("method", get, "PUT"),
        ("override", changed_query(get, "response-content-type", "text/plain"), "GET"),
        ("path", get.replace("/" + bucket + "/", "/" + bucket + "/changed/", 1), "GET"),
    ]:
        record("tampered_" + name, request(opener, url, method, body if method == "PUT" else None),
               403, "SignatureDoesNotMatch")
    assert request(opener, get)[2] == body
    old = datetime.now(timezone.utc) - timedelta(minutes=10)
    with patch("botocore.auth.get_current_datetime", return_value=old):
        expired = client.generate_presigned_url("get_object", Params={"Bucket": bucket, "Key": key}, ExpiresIn=60)
    record("expired_url", request(opener, expired), 403, "AccessDenied")

    conditional_key = "contract/conditional"
    conditional = signed("put_object", conditional_key, ContentType="application/octet-stream",
                         ContentLength=len(body), IfNoneMatch="*")
    headers = {"Content-Type": "application/octet-stream", "If-None-Match": "*"}
    record("changed_content_type", request(opener, conditional, "PUT", body,
           {**headers, "Content-Type": "text/plain"}), 403, "SignatureDoesNotMatch")
    record("changed_condition", request(opener, conditional, "PUT", body,
           {**headers, "If-None-Match": "different"}), 403, "SignatureDoesNotMatch")
    record("changed_content_length", request(opener, conditional, "PUT", body + b"x", headers),
           403, "SignatureDoesNotMatch")
    record("conditional_create", request(opener, conditional, "PUT", body, headers), 200, headers=("etag",))
    record("conditional_duplicate", request(opener, conditional, "PUT", b"x" * len(body), headers),
           412, "PreconditionFailed")
    assert request(opener, signed("get_object", conditional_key))[2] == body

    multi_key = "contract/multipart"
    result = request(opener, signed("create_multipart_upload", multi_key), "POST", b"")
    expect(result, 200)
    upload_id = xml_value(result[2], "UploadId")
    assert upload_id
    parts, multipart_body = [], b""
    for number, part in [(1, b"m" * (5 * MIB)), (2, b"last-part")]:
        multipart_body += part
        url = signed("upload_part", multi_key, UploadId=upload_id, PartNumber=number)
        result = record(f"part_{number}", request(opener, url, "PUT", part), 200, headers=("etag",))
        parts.append((number, result[1]["etag"]))
        record(f"part_{number}_retry", request(opener, url, "PUT", part), 200, headers=("etag",))
    complete = signed("complete_multipart_upload", multi_key, UploadId=upload_id)
    xml = ("<CompleteMultipartUpload>" + "".join(
        f"<Part><PartNumber>{number}</PartNumber><ETag>{etag}</ETag></Part>"
        for number, etag in parts) + "</CompleteMultipartUpload>").encode()
    record("complete_invalid_part", request(opener, complete, "POST", xml.replace(parts[-1][1].encode(), b'"wrong"')),
           400, "InvalidPart")
    result = request(opener, complete, "POST", xml)
    expect(result, 200)
    assert xml_value(result[2], "ETag")
    cases["complete"] = {"status": result[0], "etag": xml_value(result[2], "ETag")}
    result = record("multipart_get", request(opener, signed("get_object", multi_key)), 200,
                    headers=("etag", "content-length"))
    assert result[2] == multipart_body

    abort_key = "contract/abort"
    result = request(opener, signed("create_multipart_upload", abort_key), "POST", b"")
    expect(result, 200)
    upload_id = xml_value(result[2], "UploadId")
    differences["abort_other_key"] = observe(request(opener,
        signed("abort_multipart_upload", "contract/wrong-key", UploadId=upload_id), "DELETE"))
    url = signed("upload_part", abort_key, UploadId=upload_id, PartNumber=1)
    record("abort_wrong_key_preserves_session", request(opener, url, "PUT", b"still-alive"), 200, headers=("etag",))
    record("abort", request(opener, signed("abort_multipart_upload", abort_key, UploadId=upload_id), "DELETE"), 204)
    record("part_after_abort", request(opener, url, "PUT", b"still-alive"), 404, "NoSuchUpload")
    for index, item in enumerate([*keys, conditional_key, multi_key]):
        record(f"delete_{index}", request(opener, signed("delete_object", item), "DELETE"), 204)
        record(f"get_after_delete_{index}", request(opener, signed("get_object", item)), 404, "NoSuchKey")
    return {"cases": cases, "differences": differences}


def compare(reference, actual):
    assert reference["cases"], "no contract cases ran"
    assert reference["cases"].keys() == actual["cases"].keys(), "contract cases differ"
    mismatches = {name: {"minio": expected, "grove": actual["cases"][name]}
                  for name, expected in reference["cases"].items() if expected != actual["cases"][name]}
    assert not mismatches, mismatches
    # These documented exclusions are observed explicitly, not normalized away.
    names = {"suffix_range", "abort_other_key", "double_slash_key", "range_past_end_headers", "range_empty_headers"}
    assert reference["differences"].keys() == actual["differences"].keys() == names, "undocumented differences"
    assert reference["differences"]["suffix_range"]["status"] == 206
    assert actual["differences"]["suffix_range"]["status"] == 200
    assert reference["differences"]["abort_other_key"]["status"] == 204
    assert actual["differences"]["abort_other_key"]["status"] == 404
    assert actual["differences"]["abort_other_key"]["error_code"] == "NoSuchUpload"
    assert reference["differences"]["double_slash_key"]["status"] == 400
    assert reference["differences"]["double_slash_key"]["error_code"] == "XMinioInvalidObjectName"
    assert actual["differences"]["double_slash_key"]["status"] == 200
    for name in ("range_past_end_headers", "range_empty_headers"):
        assert reference["differences"][name]["headers"]["content-range"] is None
        assert actual["differences"][name]["headers"]["content-range"].startswith("bytes */")
    return len(reference["cases"])
