"""Guard the dashboard's state aggregation and native Grafana layout."""

import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ObservabilityTests(unittest.TestCase):
    def test_dashboard_panels_are_nonoverlapping_and_use_shared_datasource(self):
        dashboard = json.loads((ROOT / "deploy/observability/grafana.json").read_text())
        self.assertEqual(dashboard["uid"], "grove-storage")
        positions = []
        ids = set()
        for panel in dashboard["panels"]:
            self.assertNotIn(panel["id"], ids)
            ids.add(panel["id"])
            self.assertEqual(panel["datasource"]["uid"], "prometheus")
            position = panel["gridPos"]
            self.assertLessEqual(position["x"] + position["w"], 24)
            for other in positions:
                overlaps = (position["x"] < other["x"] + other["w"]
                            and other["x"] < position["x"] + position["w"]
                            and position["y"] < other["y"] + other["h"]
                            and other["y"] < position["y"] + position["h"])
                self.assertFalse(overlaps)
            positions.append(position)

    def test_shared_inventory_is_not_summed_and_unknown_data_is_not_zero_filled(self):
        dashboard = json.loads((ROOT / "deploy/observability/grafana.json").read_text())
        expressions = [target["expr"] for panel in dashboard["panels"] for target in panel["targets"]]
        for expression in expressions:
            self.assertNotIn("or vector(0)", expression)
            self.assertNotIn("account_id", expression)
            if "grove_objects" in expression or "grove_recovery_queue" in expression:
                self.assertIn("max by", expression)
                self.assertNotIn("sum(", expression)
        self.assertTrue(any("grove_transfer_duration_seconds_bucket" in expr for expr in expressions))
        self.assertTrue(any("grove_http_response_duration_seconds_bucket" in expr for expr in expressions))


if __name__ == "__main__":
    unittest.main()
