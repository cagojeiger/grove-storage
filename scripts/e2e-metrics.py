#!/usr/bin/env python3
"""Verify real S3 transfers and metrics against disposable PostgreSQL and MinIO."""
import json
import os
from pathlib import Path
import runpy
import socket
import time
import urllib.error
import urllib.request

from cli_management_fixture import Management
from s3_backend_fixture import minio_backend

ROOT = Path(__file__).resolve().parent
HARNESS = runpy.run_path(str(ROOT / "e2e-cli.py"))


def check(endpoint, _directory, _database, account, metrics_endpoint):
    import boto3
    from botocore.config import Config

    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def scrape():
        with opener.open(metrics_endpoint + "/metrics", timeout=5) as response:
            assert response.headers["Content-Type"].startswith("application/openmetrics-text")
            text = response.read().decode()
            assert text.endswith("# EOF\n")
            return text

    def http_count(text, method, error=None):
        labels = (f'http_request_method="{method}",url_scheme="http",'
                  'http_route="/{bucket}/{*key}",http_response_status_code="200"')
        if error:
            labels += f',error_type="{error}"'
        prefix = 'http_server_request_duration_seconds_count{' + labels + '} '
        return int(next((line.removeprefix(prefix) for line in text.splitlines()
                         if line.startswith(prefix)), "0"))

    with minio_backend() as backend:
        management = Management(endpoint, account)
        management.command("storage.create", {"id": "metrics-backend", "spec": backend.spec})
        management.command("client.create", {"id": "metrics-client", "storage_id": "metrics-backend"})
        credential = management.command("credential.create", {"client_id": "metrics-client"})
        client = boto3.client("s3", endpoint_url=endpoint, region_name="us-east-1",
                              aws_access_key_id=credential["access_key_id"],
                              aws_secret_access_key=credential["secret_key"],
                              config=Config(signature_version="s3v4", s3={"addressing_style": "path"}))
        params = {"Bucket": "metrics-client", "Key": "private-object-name"}
        payload = b"private-payload" * 1024
        put = client.generate_presigned_url("put_object", Params=params)
        with opener.open(urllib.request.Request(put, method="PUT", data=payload), timeout=10) as response:
            assert response.status == 200
        get = client.generate_presigned_url("get_object", Params=params)
        with opener.open(get, timeout=10) as response:
            assert response.read() == payload
        client.delete_object(**params)

        text = scrape()
        observed_bytes = {}
        for direction in ["receive", "send"]:
            prefix = f'grove_transfer_observed_bytes_total{{direction="{direction}"}} '
            line = next(line for line in text.splitlines() if line.startswith(prefix))
            observed_bytes[direction] = int(line.removeprefix(prefix))
            assert observed_bytes[direction] >= len(payload), direction
            assert f'grove_transfers_total{{direction="{direction}",outcome="success"}} 1' in text
        assert 'surface="s3",method="PUT",route="/{bucket}/{*key}",status_class="2xx",outcome="success"' in text
        assert "# UNIT http_server_request_duration_seconds seconds" in text
        assert "# UNIT grove_provider_call_duration_seconds seconds" in text
        assert "# UNIT grove_transfer_observed_bytes bytes" in text
        assert "grove_http_request_duration_seconds" not in text
        assert "grove_http_response_creation_duration_seconds" in text
        for method in ["PUT", "GET"]:
            assert http_count(text, method) >= 1, method
        for forbidden in ["private-object-name", "private-payload", "metrics-client", "X-Amz-", credential["secret_key"], "file_id=", "request_id="]:
            assert forbidden not in text, "sensitive or high-cardinality metric label"
        assert "grove_db_pool_connections" in text
        assert "grove_spool_reserved_bytes 0" in text
        assert "grove_log_dropped_lines_total 0" in text
        provider_operations = ["head_bucket", "list_multipart_uploads", "put_object", "get_object"]
        for operation in provider_operations:
            prefix = f'grove_provider_calls_total{{operation="{operation}",status_class="2xx",outcome="success"}} '
            assert int(next(line for line in text.splitlines() if line.startswith(prefix)).removeprefix(prefix)) > 0
            assert f'grove_provider_calls_in_flight{{operation="{operation}"}} 0' in text

        before = http_count(text, "GET")
        empty_params = {**params, "Key": params["Key"] + "-empty"}
        put = client.generate_presigned_url("put_object", Params=empty_params)
        with opener.open(urllib.request.Request(put, method="PUT", data=b""), timeout=10) as response:
            assert response.status == 200
        get = client.generate_presigned_url("get_object", Params=empty_params)
        with opener.open(get, timeout=10) as response:
            assert response.read() == b""
        client.delete_object(**empty_params)
        text = scrape()
        assert http_count(text, "GET") == before + 1, "empty GET was not recorded as complete"
        assert http_count(text, "GET", "interrupted") == 0, "empty GET was classified as interrupted"

        deadline = time.monotonic() + 10
        while True:
            text = scrape()
            prefix = 'grove_worker_last_success_timestamp_seconds{worker="management",task="retention"} '
            line = next(line for line in text.splitlines() if line.startswith(prefix))
            if int(line.removeprefix(prefix)) > 0:
                break
            if time.monotonic() > deadline:
                raise AssertionError("no completed maintenance pass observed")
            time.sleep(.1)
        deadline = time.monotonic() + 15
        while True:
            text = scrape()
            prefix = "grove_database_snapshot_timestamp_seconds "
            line = next(line for line in text.splitlines() if line.startswith(prefix))
            snapshot_at = int(line.removeprefix(prefix))
            if snapshot_at > 0:
                break
            if time.monotonic() > deadline:
                raise AssertionError("no complete database snapshot observed")
            time.sleep(.1)
        assert 'grove_object_backlog_items{queue="purge"}' in text
        assert 'grove_retention_overdue_rows{stream="security_events"}' in text
        assert 'grove_retention_seconds{stream="audit_events"} 31536000' in text
        assert 'grove_db_pool_max_connections{pool="observations"} 1' in text
        worker_prefix = 'grove_worker_runs_total{worker="database",task="snapshot",outcome="success"} '
        assert int(next(line for line in text.splitlines() if line.startswith(worker_prefix)).removeprefix(worker_prefix)) >= 1
        data_prefix = 'grove_db_table_data_bytes{schema="management",table="accounts"} '
        table_bytes = int(next(line for line in text.splitlines() if line.startswith(data_prefix)).removeprefix(data_prefix))
        assert table_bytes > 0
        assert len([line for line in text.splitlines() if line.startswith("grove_db_table_data_bytes{")]) == 22
        for forbidden in ["private-object-name", "private-payload", "metrics-client", credential["secret_key"]]:
            assert forbidden not in text
        try:
            with opener.open(endpoint + "/metrics", timeout=5):
                raise AssertionError("application listener exposed metrics")
        except urllib.error.HTTPError as error:
            assert error.code == 404
            assert b"grove_build_info" not in error.read()
        print("METRICS", json.dumps({"observed_bytes": observed_bytes,
            "spool_reserved_bytes": 0, "log_dropped_lines_total": 0,
            "maintenance_completed": True, "database_snapshot_completed": True,
            "empty_get_completed": True,
            "provider_operations": provider_operations,
            "observed_tables": 22, "account_table_data_bytes": table_bytes}, sort_keys=True))
    print("PASS real presigned PUT/GET, live counters, worker timestamp and metrics privacy/isolation")


def verify_shutdown(log):
    events = [json.loads(line).get("event_name") for line in log.splitlines()]
    assert events.count("grove.shutdown.complete") == 1
    assert "grove.shutdown.timed_out" not in events


if __name__ == "__main__":
    os.environ["RUST_LOG"] = "info"
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    bind = f"127.0.0.1:{port}"
    HARNESS["main"](lambda endpoint, directory, database, account:
        check(endpoint, directory, database, account, "http://" + bind),
        with_database=True, metrics_bind=bind, verify_log=verify_shutdown)
    with socket.socket() as probe:
        assert probe.connect_ex(("127.0.0.1", port)) != 0, "metrics listener survived shutdown"
    print("PASS metrics listener drains with the application server")
