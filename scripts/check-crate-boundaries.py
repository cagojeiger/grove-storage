"""Check production dependency ownership; dev-only fixtures are not runtime edges."""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
ALLOWED = {
    "grove-management-policy": set(),
    "grove-management-command": {"grove-management-policy"},
    "grove-object-policy": set(),
    "grove-object-service": set(),
    "grove-s3-protocol": set(),
    "filegate-core": {"grove-object-policy"},
    "filegate-db": {"grove-management-policy"},
    "grove-management-service": {
        "filegate-core", "filegate-db", "grove-management-policy", "grove-management-command",
    },
    "grove-storage-provider": {"filegate-core"},
    "filegate-infra": {"filegate-core", "grove-storage-provider", "grove-object-policy"},
    "gscli": {"grove-management-command"},
    "filegate-api": {
        "grove-management-service", "grove-management-command", "grove-management-policy",
        "grove-s3-protocol", "grove-object-service", "grove-object-policy",
        "filegate-core", "filegate-db", "filegate-infra",
    },
}
PURE = {
    "grove-management-policy", "grove-management-command", "grove-object-policy",
    "grove-object-service", "grove-s3-protocol",
}


def violations(metadata):
    members = set(metadata["workspace_members"])
    packages = [package for package in metadata["packages"] if package["id"] in members]
    names = {package["name"] for package in packages}
    errors = []
    for package in packages:
        name = package["name"]
        if name not in ALLOWED:
            errors.append(f"{name}: declare its ownership before adding a workspace crate")
            continue
        for dependency in package["dependencies"]:
            if dependency.get("kind") == "dev":
                continue
            target = dependency["name"]
            if target in names and target not in ALLOWED[name]:
                errors.append(f"{name} -> {target}: forbidden production dependency")
            provider_sdk = target.startswith(("aws-sdk-", "aws-smithy-"))
            if provider_sdk and name != "grove-storage-provider":
                errors.append(f"{name} -> {target}: provider SDK belongs to grove-storage-provider")
            if (name in PURE or name in {"grove-storage-provider", "filegate-core", "filegate-infra"}) and target in {
                "sqlx", "axum", "reqwest",
            }:
                errors.append(f"{name} -> {target}: database/HTTP adapters belong outside this crate")
    for missing in ALLOWED.keys() - names:
        errors.append(f"{missing}: expected workspace crate is missing")
    return errors


def workspace_metadata():
    return json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"], cwd=ROOT,
    ))


def main():
    errors = violations(workspace_metadata())
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("PASS production crate boundaries; provider SDK, management, and object control are separated")
    return 0


if __name__ == "__main__":
    sys.exit(main())
