"""Bind verified release evidence to source, workflow revision and trusted CI."""

import argparse
import json
import os
from pathlib import Path
import re

PREDICATE_TYPE = "https://github.com/cagojeiger/grove-storage/attestations/release-evidence/v1"


def predicate(env):
    source = env["RELEASE_SHA"]
    workflow = env["GITHUB_WORKFLOW_SHA"]
    if any(not re.fullmatch(r"[a-f0-9]{40}", sha) for sha in (source, workflow)):
        raise ValueError("Expected full source and workflow commit SHAs")
    if env["GITHUB_EVENT_NAME"] != "workflow_run" or env["RUNNER_ENVIRONMENT"] != "github-hosted":
        raise ValueError("Release requires trusted CI and a GitHub-hosted runner")
    repository = f'{env["GITHUB_SERVER_URL"]}/{env["GITHUB_REPOSITORY"]}'
    return {
        "source": {"repository": repository, "commit": source},
        "ci": {"runId": env["CI_RUN_ID"], "runAttempt": env["CI_RUN_ATTEMPT"]},
        "workflow": {"ref": env["GITHUB_WORKFLOW_REF"], "commit": workflow},
        "invocation": {
            "runId": env["GITHUB_RUN_ID"], "runAttempt": env["GITHUB_RUN_ATTEMPT"],
        },
    }


def verify(results, expected):
    if not isinstance(results, list) or not results:
        raise ValueError("Missing verified attestations")
    for result in results:
        statement = result.get("verificationResult", {}).get("statement", {})
        camel = statement.get("predicateType")
        snake = statement.get("predicate_type")
        if camel is not None and snake is not None and camel != snake:
            raise ValueError("Conflicting predicate type fields")
        if (camel or snake) == PREDICATE_TYPE and statement.get("predicate") == expected:
            return
    raise ValueError("No verified evidence matches source, workflow, CI and release run")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("generate", "verify"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    expected = predicate(os.environ)
    if args.command == "generate":
        args.path.write_text(json.dumps(expected, indent=2) + "\n")
    else:
        verify(json.loads(args.path.read_text()), expected)


if __name__ == "__main__":
    main()
