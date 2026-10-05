#!/usr/bin/env python3
"""Verify the real server finishes SIGTERM shutdown against a disposable DB."""
import json
import os
from pathlib import Path
import runpy

HARNESS = runpy.run_path(str(Path(__file__).with_name("e2e-cli.py")))


def verify(log):
    events = [json.loads(line).get("fields", {}).get("event") for line in log.splitlines()]
    assert events.count("server.shutting_down") == 1, events
    assert events.count("shutdown.complete") == 1, events
    assert "shutdown.timed_out" not in events, events
    assert "background.join_failed" not in events, events
    assert "reconciler.join_failed" not in events, events
    print("PASS real SIGTERM drains HTTP/background workers and closes both DB pools")


if __name__ == "__main__":
    os.environ["RUST_LOG"] = "info"
    HARNESS["main"](lambda *_: None, with_database=True, verify_log=verify)
