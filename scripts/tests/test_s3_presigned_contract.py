from copy import deepcopy
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from s3_presigned_contract import NoRedirect, compare, observe, xml_value


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.reference = {"cases": {"get": observe((200, {"etag": '"abc"'}, b"object"), headers=("etag",))},
            "differences": {"suffix_range": {"status": 206}, "abort_other_key": {"status": 204},
                            "double_slash_key": {"status": 400, "error_code": "XMinioInvalidObjectName"}}}
        self.actual = deepcopy(self.reference)
        self.actual["differences"] = {"suffix_range": {"status": 200},
            "abort_other_key": {"status": 404, "error_code": "NoSuchUpload"}, "double_slash_key": {"status": 200}}
        for name in ("range_past_end_headers", "range_empty_headers"):
            self.reference["differences"][name] = {"headers": {"content-range": None}}
            self.actual["differences"][name] = {"headers": {"content-range": "bytes */0"}}

    def test_same_contract_counts_every_case(self):
        self.assertEqual(compare(self.reference, self.actual), 1)

    def test_body_status_and_selected_headers_are_not_normalized_away(self):
        for field, changed in [("status", 206), ("sha256", "wrong"), ("headers", {"etag": '"wrong"'}),
                               ("error_code", "SignatureDoesNotMatch")]:
            with self.subTest(field=field):
                actual = deepcopy(self.actual)
                actual["cases"]["get"][field] = changed
                with self.assertRaises(AssertionError):
                    compare(self.reference, actual)

    def test_missing_or_extra_cases_fail(self):
        for cases in [{}, {**self.actual["cases"], "extra": {}}]:
            with self.assertRaises(AssertionError):
                compare(self.reference, {**self.actual, "cases": cases})

    def test_undocumented_difference_fails(self):
        self.actual["differences"]["suffix_range"]["status"] = 500
        with self.assertRaises(AssertionError):
            compare(self.reference, self.actual)

    def test_zero_cases_cannot_pass(self):
        with self.assertRaises(AssertionError):
            compare({**self.reference, "cases": {}}, {**self.actual, "cases": {}})

    def test_extra_exclusion_cannot_hide_a_new_difference(self):
        self.reference["differences"]["new"] = {"status": 200}
        self.actual["differences"]["new"] = {"status": 500}
        with self.assertRaises(AssertionError):
            compare(self.reference, self.actual)

    def test_volatile_headers_are_excluded_but_requested_headers_remain(self):
        first = observe((200, {"date": "first", "x-amz-request-id": "first", "etag": "same"}, b"body"), headers=("etag",))
        second = observe((200, {"date": "second", "x-amz-request-id": "second", "etag": "same"}, b"body"), headers=("etag",))
        self.assertEqual(first, second)
        self.assertEqual(first["headers"], {"etag": "same"})
        self.assertNotIn("body", str(first))

    def test_xml_error_code_is_compared_without_volatile_error_body(self):
        first = observe((403, {}, b"<Error><Code>AccessDenied</Code><RequestId>one</RequestId></Error>"))
        second = observe((403, {}, b"<Error><Code>AccessDenied</Code><RequestId>two</RequestId></Error>"))
        self.assertEqual(first, second)
        self.assertEqual(first["error_code"], "AccessDenied")
        second["error_code"] = "InvalidAccessKeyId"
        self.assertNotEqual(first, second)

    def test_namespaced_xml_and_head_error_without_body(self):
        self.assertEqual(xml_value(b'<Error xmlns="urn:s3"><Code>NoSuchKey</Code></Error>', "Code"), "NoSuchKey")
        self.assertIsNone(observe((404, {}, b""))["error_code"])

    def test_redirects_are_not_followed(self):
        self.assertIsNone(NoRedirect().redirect_request(None, None, 307, "redirect", {}, "https://other.test"))


if __name__ == "__main__":
    unittest.main()
