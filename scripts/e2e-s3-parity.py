#!/usr/bin/env python3
"""Compare one presigned consumer contract against isolated MinIO and Grove."""

import argparse
from contextlib import nullcontext
import json
from pathlib import Path
import runpy
import ssl
import subprocess
import urllib.request

import boto3
import botocore

from cli_management_fixture import Management
from s3_backend_fixture import minio_backend
from s3_credential_contract import check_credentials
from s3_presigned_contract import NoRedirect, compare, run_contract, sdk
from s3_tls_fixture import https_fixture

ROOT = Path(__file__).resolve().parent
HARNESS = runpy.run_path(str(ROOT / "e2e-cli.py"))


def check(endpoint, directory, database, account, backend, page_origin, report_dir):
    management = Management(endpoint, account)
    management.command("storage.create", {"id": "parity-storage", "spec": backend.spec})
    management.command("client.create", {"id": "parity-client", "storage_id": "parity-storage"})
    credential = management.command("credential.create", {"client_id": "parity-client"})
    vendor_key = {"access_key_id": backend.spec["access_key"], "secret_key": backend.spec["secret_key"]}
    reference_bucket = "parity-reference"
    backend.vendor.create_bucket(Bucket=reference_bucket)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    report = {"status": "running", "sdk": {"boto3": boto3.__version__, "botocore": botocore.__version__},
              "grove_revision": subprocess.check_output(["git", "rev-parse", "HEAD"],
                  cwd=ROOT.parent, text=True).strip(),
              "minio_revision": "07c3a429bfed433e49018cb0f78a52145d4bedeb",
              "production_endpoints": False, "transports": {}}

    def save():
        (report_dir / "contract.json").write_text(json.dumps(report, indent=2))

    def run_pair(transport, reference_endpoint, grove_endpoint, reference_cert=True, grove_cert=True):
        result = report["transports"][transport] = {}
        result["minio"] = run_contract(sdk(reference_endpoint, vendor_key, reference_cert), reference_bucket, opener)
        save()
        result["grove"] = run_contract(sdk(grove_endpoint, credential, grove_cert), "parity-client", opener)
        save()
        result["matching_cases"] = compare(result["minio"], result["grove"])
        print("PASS", transport, result["matching_cases"], "matching presigned cases;",
              len(result["minio"]["differences"]), "explicit difference observations")

    try:
        run_pair("http", backend.endpoint, endpoint)
        if page_origin:
            with https_fixture(backend.endpoint) as (reference_tls, reference_cert), \
                 https_fixture(endpoint) as (grove_tls, grove_cert):
                context = ssl.create_default_context(cafile=str(reference_cert))
                context.load_verify_locations(cafile=str(grove_cert))
                opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect(),
                    urllib.request.HTTPSHandler(context=context))
                run_pair("https_proxy", reference_tls, grove_tls, str(reference_cert), str(grove_cert))
                from s3_browser_contract import check_browser
                report["browser"] = check_browser(page_origin, [
                    ("minio", sdk(reference_tls, vendor_key, str(reference_cert)), reference_bucket),
                    ("grove", sdk(grove_tls, credential, str(grove_cert)), "parity-client"),
                ], report_dir)
        report["credentials"] = check_credentials(management, endpoint, credential, "parity-client", opener)
        report["status"] = "passed"
    except Exception as error:
        report["status"] = "failed"
        report["failure"] = {"type": type(error).__name__}
        if isinstance(error, AssertionError):
            report["failure"]["detail"] = str(error)
        raise
    finally:
        save()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--browser", action="store_true", help="Also compare HTTPS and real Chromium CORS transfers")
    parser.add_argument("--report-dir", type=Path, default=ROOT.parent / "reports/s3-parity")
    options = parser.parse_args()
    options.report_dir.mkdir(parents=True, exist_ok=True)
    with https_fixture() if options.browser else nullcontext((None, None)) as (origin, _cert):
        with minio_backend(cors_origin=origin) as backend:
            HARNESS["main"](lambda endpoint, directory, database, account:
                check(endpoint, directory, database, account, backend, origin, options.report_dir),
                with_database=True, s3_cors_origins=(origin,) if origin else ())
