#!/usr/bin/env python3
"""Verify observed presigned transfers and an exporter independent of PostgreSQL."""

from pathlib import Path
import runpy
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parent
HARNESS = runpy.run_path(str(ROOT / "e2e-cli.py"))


def check(endpoint, directory, database, account, metrics_endpoint, backend):
    import boto3
    from botocore.config import Config
    from prometheus_client.parser import text_string_to_metric_families
    from cli_management_fixture import Management

    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def scrape():
        with opener.open(metrics_endpoint + "/metrics", timeout=5) as response:
            assert response.status == 200
            assert response.headers["Content-Type"].startswith("text/plain; version=0.0.4")
            assert response.headers["Cache-Control"] == "no-store"
            text = response.read().decode()
        samples = [sample for family in text_string_to_metric_families(text) for sample in family.samples]
        for sample in samples:
            assert set(sample.labels) <= {
                "surface", "operation", "outcome", "direction", "reason", "state",
                "pool", "worker", "task", "stream", "source", "version", "le",
            }, sample.labels
        return text, samples

    def value(samples, name, **labels):
        matches = [sample.value for sample in samples if sample.name == name
                   and all(sample.labels.get(key) == val for key, val in labels.items())]
        assert len(matches) == 1, (name, labels, matches)
        return matches[0]

    try:
        opener.open(endpoint + "/metrics", timeout=5)
    except urllib.error.HTTPError as error:
        assert error.code == 404
    else:
        raise AssertionError("the public API listener exposed metrics")
    management = Management(endpoint, account)
    management.command("storage.create", {"id": "metrics-private-storage", "spec": backend.spec})
    management.command("client.create", {"id": "metrics-private-client", "storage_id": "metrics-private-storage"})
    credential = management.command("credential.create", {"client_id": "metrics-private-client"})
    client = boto3.client(
        "s3", endpoint_url=endpoint, region_name="us-east-1",
        aws_access_key_id=credential["access_key_id"], aws_secret_access_key=credential["secret_key"],
        config=Config(signature_version="s3v4", s3={"addressing_style": "path"}),
    )
    key = "metrics-private-object"
    data = b"presigned-observed-transfer"
    for method, operation, parameters, body, expected in [
        ("PUT", "put_object", {"Bucket": "metrics-private-client", "Key": key}, data, None),
        ("GET", "get_object", {"Bucket": "metrics-private-client", "Key": key}, None, data),
        ("GET", "get_object", {"Bucket": "metrics-private-client", "Key": key, "Range": "bytes=0-3"}, None, data[:4]),
        ("HEAD", "head_object", {"Bucket": "metrics-private-client", "Key": key}, None, b""),
    ]:
        url = client.generate_presigned_url(operation, Params=parameters, ExpiresIn=60)
        headers = {"Range": "bytes=0-3"} if "Range" in parameters else {}
        with opener.open(urllib.request.Request(url, method=method, data=body, headers=headers), timeout=10) as response:
            result = response.read()
            if expected is not None:
                assert result == expected
    client.delete_object(Bucket="metrics-private-client", Key=key)
    text, samples = scrape()
    assert value(samples, "grove_http_requests_total", surface="s3", operation="put", outcome="2xx") == 1
    assert value(samples, "grove_http_requests_total", surface="s3", operation="get", outcome="2xx") == 2
    assert value(samples, "grove_transfer_bytes_total", direction="receive") == len(data)
    assert value(samples, "grove_transfer_bytes_total", direction="send") == len(data) + 4
    assert value(samples, "grove_transfers_total", direction="send", outcome="success") == 2
    assert value(samples, "grove_transfer_in_flight", direction="send") == 0
    assert value(samples, "grove_transfer_in_flight", direction="receive") == 0
    assert value(samples, "grove_provider_requests_total", operation="put", outcome="success") == 1
    assert value(samples, "grove_management_operations_total", surface="resource_api", operation="client.create", outcome="success") == 1
    for private in [key, "metrics-private-client", "metrics-private-storage", credential["access_key_id"],
                    credential["secret_key"], management.token, backend.spec["endpoint"]]:
        assert private not in text
    print("PASS presigned PUT/GET/Range/HEAD/DELETE retain their wire contract and bounded metric labels")

    deadline = time.monotonic() + 10
    while value(samples, "grove_snapshot_available", source="db") != 1:
        if time.monotonic() >= deadline:
            raise AssertionError("initial database snapshot did not succeed")
        time.sleep(0.1)
        _, samples = scrape()
    previous = value(samples, "grove_snapshot_last_success_timestamp_seconds", source="db")
    HARNESS["docker"]("stop", database)
    deadline = time.monotonic() + 75
    while True:
        _, samples = scrape()
        if value(samples, "grove_snapshot_available", source="db") == 0:
            break
        if time.monotonic() >= deadline:
            raise AssertionError("DB failure was not reflected in the cached snapshot")
        time.sleep(1)
    assert value(samples, "grove_snapshot_errors_total", source="db") >= 1
    assert value(samples, "grove_snapshot_last_success_timestamp_seconds", source="db") >= previous > 0
    print("PASS exporter remains available without PostgreSQL; failed snapshots are not healthy zeros")


if __name__ == "__main__":
    from s3_backend_fixture import minio_backend
    with minio_backend() as backend:
        HARNESS["main"](
            lambda endpoint, directory, database, account, metrics_endpoint:
                check(endpoint, directory, database, account, metrics_endpoint, backend),
            with_database=True, with_metrics=True,
        )
