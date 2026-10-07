import copy
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location(
    "crate_boundaries", Path(__file__).resolve().parents[1] / "check-crate-boundaries.py",
)
boundaries = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(boundaries)


class CrateBoundaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.metadata = boundaries.workspace_metadata()

    def add_dependency(self, source, target, kind=None):
        metadata = copy.deepcopy(self.metadata)
        package = next(package for package in metadata["packages"] if package["name"] == source)
        package["dependencies"].append({"name": target, "kind": kind})
        return boundaries.violations(metadata)

    def test_current_workspace_obeys_boundaries(self):
        self.assertEqual(boundaries.violations(self.metadata), [])

    def test_object_control_cannot_depend_on_management_or_provider(self):
        for target in ["grove-management-service", "grove-storage-provider", "grove-db"]:
            with self.subTest(target=target):
                self.assertTrue(self.add_dependency("grove-object-service", target))

    def test_management_cannot_depend_on_object_execution(self):
        for target in ["grove-object-service", "grove-infra", "grove-storage-provider"]:
            with self.subTest(target=target):
                self.assertTrue(self.add_dependency("grove-management-service", target))

    def test_provider_and_transfer_cannot_depend_on_accounts_or_database(self):
        for source in ["grove-storage-provider", "grove-infra"]:
            for target in ["grove-management-service", "grove-db"]:
                with self.subTest(source=source, target=target):
                    self.assertTrue(self.add_dependency(source, target))

    def test_sdk_cannot_leak_back_into_transfer_or_api(self):
        for source in ["grove-infra", "grove-api"]:
            self.assertTrue(self.add_dependency(source, "aws-sdk-s3"))

    def test_test_fixtures_are_not_production_dependencies(self):
        self.assertEqual(self.add_dependency("grove-storage-provider", "axum", "dev"), [])

    def test_build_dependencies_are_checked(self):
        self.assertTrue(self.add_dependency("grove-object-service", "grove-db", "build"))

    def test_removing_a_required_crate_is_not_silent(self):
        metadata = copy.deepcopy(self.metadata)
        metadata["packages"] = [package for package in metadata["packages"]
                                if package["name"] != "grove-storage-provider"]
        self.assertTrue(boundaries.violations(metadata))


if __name__ == "__main__":
    unittest.main()
