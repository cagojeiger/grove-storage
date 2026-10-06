"""Validate image scan evidence and resolve immutable Linux platform targets."""

import argparse
import json
from pathlib import Path
import re


def validate(report):
    if not isinstance(report, dict) or report.get("SchemaVersion") != 2:
        raise ValueError("Missing or unsupported Trivy report")
    results = report.get("Results")
    if not isinstance(results, list) or not results:
        raise ValueError("Missing or empty image results")
    rust_inventory = False
    findings = 0
    for result in results:
        if not isinstance(result, dict):
            raise ValueError("Malformed image result")
        packages = result.get("Packages")
        if result.get("Type") == "rustbinary":
            if not isinstance(packages, list) or not packages:
                raise ValueError("Empty Rust dependency inventory")
            if any(not isinstance(package, dict) or not package.get("Name")
                   or not package.get("Version") for package in packages):
                raise ValueError("Malformed Rust dependency inventory")
            rust_inventory = True
        for kind in ("Vulnerabilities", "Secrets"):
            entries = result.get(kind)
            if entries is None:
                entries = []
            if not isinstance(entries, list):
                raise ValueError(f"Malformed {kind}")
            for entry in entries:
                if not isinstance(entry, dict) or entry.get("Severity") not in {
                    "UNKNOWN", "LOW", "MEDIUM", "HIGH", "CRITICAL",
                }:
                    raise ValueError(f"Malformed {kind} finding")
                findings += entry["Severity"] in {"HIGH", "CRITICAL"}
    if not rust_inventory:
        raise ValueError("No Rust inventory; build with cargo-auditable")
    if findings:
        raise ValueError(f"{findings} HIGH/CRITICAL vulnerability or secret findings")


def platform_digest(manifest, architecture):
    if not isinstance(manifest, dict) or not isinstance(manifest.get("manifests"), list):
        raise ValueError("Expected an OCI image index")
    matches = []
    for item in manifest["manifests"]:
        if not isinstance(item, dict) or not isinstance(item.get("platform"), dict):
            raise ValueError("Malformed platform descriptor")
        platform = item["platform"]
        if platform.get("os") == "linux" and platform.get("architecture") == architecture:
            matches.append(item.get("digest"))
    if len(matches) != 1 or not isinstance(matches[0], str) or not re.fullmatch(
        r"sha256:[a-f0-9]{64}", matches[0]
    ):
        raise ValueError(f"Expected exactly one immutable linux/{architecture} image")
    return matches[0]


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    scan = commands.add_parser("validate")
    scan.add_argument("path", type=Path)
    platform = commands.add_parser("platform")
    platform.add_argument("path", type=Path)
    platform.add_argument("architecture", choices=("amd64", "arm64"))
    args = parser.parse_args()
    data = json.loads(args.path.read_text())
    if args.command == "validate":
        validate(data)
    else:
        print(platform_digest(data, args.architecture))


if __name__ == "__main__":
    main()
