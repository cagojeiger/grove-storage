#!/usr/bin/env python3
"""Initialize or recover a disposable account through the real local TTY command."""

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
SERVER = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve() / "debug/grove-storage"


def run_tty(command, password, env=None):
    pid, terminal = pty.fork()
    if pid == 0:
        os.execvpe(command[0], command, os.environ if env is None else env)
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
            raise RuntimeError("local account command timed out")
    finally:
        os.close(terminal)
    _, status = os.waitpid(pid, 0)
    if status != 0 or prompts:
        raise RuntimeError("local account command failed")
    result = json.loads(output.decode().splitlines()[-1])
    return result


def main():
    if len(sys.argv) not in (2, 3):
        raise SystemExit("usage: e2e-password-account.py <container> [account-id]")
    password = os.environ["GROVE_E2E_PASSWORD"]
    port = subprocess.check_output(
        ["docker", "port", sys.argv[1], "5432"], text=True, timeout=10
    ).strip().rsplit(":", 1)[1]
    env = dict(os.environ)
    env["GROVE_DATABASE_URL"] = f"postgres://grove:grove@127.0.0.1:{port}/grove"
    command = (["recover", sys.argv[2], os.environ.get("GROVE_E2E_USERNAME", "owner"), "--yes"] if len(sys.argv) == 3 else
               ["init", "owner", os.environ.get("GROVE_E2E_DISPLAY_NAME", "Fixture owner")])
    result = run_tty([str(SERVER), "account", *command], password, env)
    if len(sys.argv) == 3 and result["account_id"] != sys.argv[2]:
        raise RuntimeError("local recovery changed the account identity")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
