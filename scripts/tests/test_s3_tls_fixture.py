from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import ssl
import sys
import threading
import unittest
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from s3_tls_fixture import https_fixture


class TLSProxyTests(unittest.TestCase):
    def test_preserves_signed_request_target_host_and_head_length(self):
        received = []

        class Backend(BaseHTTPRequestHandler):
            def reply(self):
                received.append((self.command, self.path, self.headers["Host"]))
                self.send_response(200)
                self.send_header("Content-Length", "5")
                self.send_header("ETag", '"test-etag"')
                self.end_headers()
                if self.command != "HEAD":
                    self.wfile.write(b"bytes")

            do_GET = do_HEAD = do_OPTIONS = reply

            def log_message(self, *_args):
                pass

        backend = ThreadingHTTPServer(("127.0.0.1", 0), Backend)
        thread = threading.Thread(target=backend.serve_forever, daemon=True)
        thread.start()
        try:
            with https_fixture(f"http://127.0.0.1:{backend.server_port}") as (origin, certificate):
                opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                    urllib.request.HTTPSHandler(context=ssl.create_default_context(cafile=str(certificate))))
                path = "/bucket/a%252Fb+%20file?X-Amz-Signature=abc&response-content-type=a%2Fb"
                for method in ("GET", "HEAD", "OPTIONS"):
                    with opener.open(urllib.request.Request(origin + path, method=method), timeout=5) as response:
                        self.assertEqual(response.headers["Content-Length"], "5")
                        self.assertEqual(response.headers["ETag"], '"test-etag"')
                        self.assertEqual(response.read(), b"" if method == "HEAD" else b"bytes")
                self.assertEqual(received, [(method, path, origin.removeprefix("https://"))
                                           for method in ("GET", "HEAD", "OPTIONS")])
        finally:
            backend.shutdown()
            thread.join(timeout=5)
            backend.server_close()

    def test_refuses_non_fixture_upstreams(self):
        for endpoint in ["https://127.0.0.1:9000", "http://production.example:9000"]:
            with self.assertRaises(ValueError):
                with https_fixture(endpoint):
                    self.fail("external upstream was accepted")


if __name__ == "__main__":
    unittest.main()
