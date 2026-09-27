"""Conditional single PUT contracts, including NoteGate-style presigning."""

from concurrent.futures import ThreadPoolExecutor
from threading import Barrier
import urllib.error
import urllib.request

from botocore.exceptions import ClientError


def check_conditional_put(client, opener):
    args = {"Bucket": "s3-test", "Key": "conditional-presigned"}
    body = b"notegate-presigned-body"
    url = client.generate_presigned_url("put_object", Params={
        **args, "ContentType": "application/octet-stream",
        "ContentLength": len(body), "IfNoneMatch": "*",
    })
    request = urllib.request.Request(url, method="PUT", data=body, headers={
        "Content-Type": "application/octet-stream", "If-None-Match": "*",
    })
    with opener.open(request, timeout=10) as response:
        assert response.status == 200 and response.headers["ETag"]
    try:
        opener.open(request, timeout=10)
    except urllib.error.HTTPError as error:
        assert error.code == 412 and b"PreconditionFailed" in error.read()
    else:
        raise AssertionError("presigned conditional retry overwrote an existing key")
    assert client.get_object(**args)["Body"].read() == body
    client.delete_object(**args)
    with opener.open(request, timeout=10) as response:
        assert response.status == 200
    client.delete_object(**args)
    print("PASS presigned conditional PUT, duplicate 412, and create after deletion")

    args["Key"] = "conditional-race"
    barrier = Barrier(8)

    def put(index):
        content = f"writer-{index}".encode()
        barrier.wait(timeout=10)
        try:
            client.put_object(**args, Body=content, IfNoneMatch="*")
            return content
        except ClientError as error:
            assert error.response["ResponseMetadata"]["HTTPStatusCode"] == 412, error
            assert error.response["Error"]["Code"] == "PreconditionFailed", error
            return None

    with ThreadPoolExecutor(max_workers=8) as executor:
        winners = [result for result in executor.map(put, range(8)) if result is not None]
    assert len(winners) == 1, winners
    assert client.get_object(**args)["Body"].read() == winners[0]
    client.put_object(**args, Body=b"ordinary-overwrite")
    assert client.get_object(**args)["Body"].read() == b"ordinary-overwrite"
    client.delete_object(**args)
    print("PASS eight concurrent conditional PUTs have one winner; ordinary overwrite unchanged")
