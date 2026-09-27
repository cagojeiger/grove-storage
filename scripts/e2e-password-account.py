#!/usr/bin/env python3
"""Recover a disposable browser account through the real local TTY command."""

import errno
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
SERVER = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve() / "debug/filegate"


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: e2e-password-account.py <container> <account-id>")
    password = os.environ["GROVE_E2E_PASSWORD"]
    port = subprocess.check_output(
        ["docker", "port", sys.argv[1], "5432"], text=True, timeout=10
    ).strip().rsplit(":", 1)[1]
    env = dict(os.environ)
    env["FILEGATE_DATABASE_URL"] = f"postgres://filegate:filegate@127.0.0.1:{port}/filegate"
    pid, terminal = pty.fork()
    if pid == 0:
        os.execve(
            SERVER,
            [str(SERVER), "account", "recover", sys.argv[2], "owner", "--yes"],
            env,
        )
    output = bytearray()
    prompts = [b"New password: ", b"Confirm password: "]
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            ready, _, _ = select.select([terminal], [], [], 0.2)
            if not ready:
                continue
            try:
                chunk = os.read(terminal, 4096)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            output.extend(chunk)
            if prompts and prompts[0] in output:
                os.write(terminal, password.encode() + b"\n")
                prompts.pop(0)
        else:
            os.kill(pid, 9)
            raise RuntimeError("local account recovery timed out")
    finally:
        os.close(terminal)
    _, status = os.waitpid(pid, 0)
    if status != 0 or prompts:
        raise RuntimeError("local account recovery failed")
    result = json.loads(output.decode().splitlines()[-1])
    if result["account_id"] != sys.argv[2]:
        raise RuntimeError("local recovery changed the account identity")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
