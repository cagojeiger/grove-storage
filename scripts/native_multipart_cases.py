"""Native API parity over direct S3 and S3 relay, using disposable fixtures."""

import hashlib
import urllib.error


def check_multipart(request, put, opener, endpoint, backend, admin):
    parts = [b"a" * (5 * 1024 * 1024), b"b" * (5 * 1024 * 1024), b"tail"]
    content = b"".join(parts)
    expected_etag = hashlib.md5(b"".join(hashlib.md5(part).digest() for part in parts)).hexdigest() + "-3"
    etags = []
    for relay in [False, True]:
        name = "native-multipart-" + ("relay" if relay else "direct")
        token = name + "-test-key"
        request("POST", "/api/admin/v1/storages",
                {"id": name, **backend.spec, "force_relay": relay}, admin, 201)
        request("POST", "/api/admin/v1/clients", {"id": name, "storage_id": name}, admin, 201)
        request("POST", f"/api/admin/v1/clients/{name}/keys",
                {"key_hash": "sha256:" + hashlib.sha256(token.encode()).hexdigest()}, admin, 201)
        created = request("POST", "/api/v1/files", {"declared_size": len(content)}, token, 201)
        file_id = created["file_id"]
        assert created["multipart"]["part_count"] == 3
        urls = request("POST", f"/api/v1/files/{file_id}/parts", {"parts": [1, 2, 3]}, token)
        urls = {part["part"]: part["url"] for part in urls["parts"]}
        for url in urls.values():
            assert url.startswith((endpoint + "/blobs/") if relay else backend.endpoint + "/")
        for number in [3, 1]:
            assert put(urls[number], parts[number - 1])[0] == 200
        try:
            request("POST", f"/api/v1/files/{file_id}/commit", token=token)
        except urllib.error.HTTPError as error:
            assert error.code == 400
        else:
            raise AssertionError("incomplete multipart was committed")
        if relay:
            assert put(urls[2], b"short")[0] == 400
        renewed = request("POST", f"/api/v1/files/{file_id}/parts", {"parts": [2]}, token)
        assert put(renewed["parts"][0]["url"], parts[1])[0] == 200
        assert put(urls[1], parts[0])[0] == 200
        committed = request("POST", f"/api/v1/files/{file_id}/commit", token=token)
        stat = request("GET", f"/api/v1/files/{file_id}", token=token)
        assert stat["state"] == "active"
        assert stat["declared_size"] == len(content)
        assert committed["etag"] == expected_etag
        etags.append(committed["etag"])
        # The short final part lets HTTP clients observe early rejection without
        # a large body write racing the server's connection close.
        assert put(urls[3], parts[2])[0] == (403 if relay else 404)
        read = request("POST", f"/api/v1/files/{file_id}/read", {}, token)
        with opener.open(read["get_url"], timeout=30) as response:
            assert response.read() == content
        request("DELETE", f"/api/v1/files/{file_id}", token=token)
        assert not backend.vendor.list_multipart_uploads(Bucket=backend.spec["bucket"]).get("Uploads")
        print(f"PASS {name}: out-of-order parts, incomplete rejection, renewal, ETag, bytes and late-part rejection")
    assert etags[0] == etags[1]
