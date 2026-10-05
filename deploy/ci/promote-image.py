"""Refuse version overwrite or ambiguous registry errors before image promotion."""

import argparse
import base64
import json
import os
import re
import subprocess
from urllib.error import HTTPError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

ACCEPT = ", ".join((
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
))


def version_digest(image, version):
    registry, repository = image.split("/", 1)
    if registry != "ghcr.io":
        raise ValueError("Only the configured GHCR release registry is supported")
    basic = base64.b64encode(
        f'{os.environ["GITHUB_ACTOR"]}:{os.environ["GH_TOKEN"]}'.encode()
    ).decode()
    query = urlencode({"service": registry, "scope": f"repository:{repository}:pull"})
    request = Request(f"https://{registry}/token?{query}",
                      headers={"Authorization": f"Basic {basic}"})
    with urlopen(request, timeout=30) as response:
        token = json.load(response)["token"]
    request = Request(f"https://{registry}/v2/{repository}/manifests/{version}",
                      method="HEAD", headers={"Authorization": f"Bearer {token}", "Accept": ACCEPT})
    try:
        with urlopen(request, timeout=30) as response:
            digest = response.headers.get("Docker-Content-Digest", "")
    except HTTPError as error:
        if error.code == 404:
            return None
        raise
    if not isinstance(digest, str) or not re.fullmatch(r"sha256:[a-f0-9]{64}", digest):
        raise ValueError("Registry returned no valid manifest digest")
    return digest


def promote(image, version, digest):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Expected a stable release version")
    if not re.fullmatch(r"sha256:[a-f0-9]{64}", digest):
        raise ValueError("Expected an immutable image digest")
    if version_digest(image, version) is not None:
        raise ValueError("Published image version already exists; use a new version")
    subprocess.run([
        "docker", "buildx", "imagetools", "create",
        "-t", f"{image}:{version}", "-t", f"{image}:latest", f"{image}@{digest}",
    ], check=True)
    for tag in (version, "latest"):
        if version_digest(image, tag) != digest:
            raise ValueError(f"Published {tag} does not match the verified digest")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("image")
    parser.add_argument("version")
    parser.add_argument("digest")
    args = parser.parse_args()
    promote(args.image, args.version, args.digest)


if __name__ == "__main__":
    main()
