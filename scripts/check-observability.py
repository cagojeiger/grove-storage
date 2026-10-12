#!/usr/bin/env python3
"""Validate dashboard expressions with the same PromQL parser as alert rules."""

import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--promtool", default="promtool")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent / "deploy" / "observability"
    dashboard = json.loads((root / "grafana.json").read_text())
    expressions = [target["expr"] for panel in dashboard["panels"] for target in panel.get("targets", [])]
    assert expressions, "dashboard has no expressions"
    rules = [{"record": f"dashboard_check_{index}", "expr": expression
              .replace("$cluster", "home-prod-kr").replace("$__rate_interval", "5m")}
             for index, expression in enumerate(expressions)]
    with tempfile.TemporaryDirectory(prefix="grove-dashboard-check-") as directory:
        path = Path(directory) / "expressions.json"
        path.write_text(json.dumps({"groups": [{"name": "dashboard-check", "rules": rules}]}))
        subprocess.run([args.promtool, "check", "rules", str(path)], check=True)
    subprocess.run([args.promtool, "check", "rules", str(root / "rules.yaml")], check=True)
    subprocess.run([args.promtool, "test", "rules", str(root / "rules.test.yaml")], check=True)
    print(f"PASS {len(expressions)} dashboard expressions and alert rule tests")


if __name__ == "__main__":
    main()
