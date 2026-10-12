#!/usr/bin/env python3
"""Read-only S3 comparison of a frozen FileGate snapshot and Grove endpoint."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import sys
import urllib.request

import boto3
from botocore.config import Config


def check(condition, message):
    if not condition:
        raise RuntimeError(message)


def client(endpoint, region):
    return boto3.client("s3", endpoint_url=endpoint, region_name=region,
        config=Config(signature_version="s3v4", s3={"addressing_style": "path"},
                      connect_timeout=5, read_timeout=30, retries={"total_max_attempts": 1}))


def expected(snapshot, bucket):
    files = {row["id"]: row for row in snapshot["tables"]["files"]}
    return {row["key"]: (files[row["file_id"]]["declared_size"],
                         files[row["file_id"]]["etag"].strip('"'),
                         files[row["file_id"]]["content_type"] or "application/octet-stream")
            for row in snapshot["tables"]["s3_keys"] if row["client_id"] == bucket}


def probe(source, target, bucket, item):
    key, (size, etag, content_type) = item
    clients = [source, target] if source is not None else [target]
    heads = [s3.head_object(Bucket=bucket, Key=key) for s3 in clients]
    for head in heads:
        check(head["ContentLength"] == size, "HEAD size differs")
        check(head["ETag"] == '"' + etag + '"', "HEAD ETag differs")
        check(head["ContentType"] == content_type, "HEAD content type differs")
    if source is not None:
        check(heads[0].get("LastModified") == heads[1].get("LastModified"), "HEAD timestamp differs")
    params = {"Bucket": bucket, "Key": key}
    if size:
        params["Range"] = f"bytes=0-{min(size, 4096) - 1}"
    data = []
    for s3 in clients:
        url = s3.generate_presigned_url("get_object", Params=params, ExpiresIn=300)
        headers = {"User-Agent": "Grove-Migration-Verification"}
        if size:
            headers["Range"] = params["Range"]
        request = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(request, timeout=30) as response:
            check(response.status == (206 if size else 200), "GET status differs")
            body = response.read(4097)
            check(len(body) == min(size, 4096), "GET size differs")
            data.append(body)
    check(all(body == data[0] for body in data), "presigned GET bytes differ")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--source", help="FileGate endpoint for before/after byte comparison")
    parser.add_argument("--target", required=True)
    parser.add_argument("--bucket", required=True)
    parser.add_argument("--region", default="us-east-1")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise ValueError("report already exists")
    snapshot = json.loads(args.snapshot.read_bytes())
    check(snapshot["format"] == "filegate-0.4.1-v1", "unsupported snapshot")
    wanted = expected(snapshot, args.bucket)
    check(wanted, "snapshot bucket has no keys")
    source = client(args.source, args.region) if args.source else None
    target = client(args.target, args.region)
    with ThreadPoolExecutor(max_workers=4) as workers:
        for count, _ in enumerate(workers.map(
                lambda item: probe(source, target, args.bucket, item), wanted.items()), 1):
            if count % 25 == 0:
                print(f"PASS HEAD / presigned Range GET: {count}/{len(wanted)}", flush=True)
    report = {"snapshot_sha256": hashlib.sha256(args.snapshot.read_bytes()).hexdigest(),
              "bucket": args.bucket, "objects": len(wanted),
              "bytes": sum(value[0] for value in wanted.values()),
              "head_presigned_get": "passed", "compared_filegate": source is not None,
              "object_writes": 0}
    descriptor = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # SDK/HTTP errors can contain signed URLs or logical object names.
        print(f"migration read verification failed: {type(error).__name__}", file=sys.stderr)
        raise SystemExit(1) from None
