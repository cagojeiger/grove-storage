"""Loopback-only test proxy that loses vendor Complete responses."""

from contextlib import contextmanager
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import socket
import threading
from urllib.parse import parse_qs, urlsplit


@contextmanager
def lose_complete_responses(endpoint):
    upstream = urlsplit(endpoint)
    assert upstream.scheme == "http" and upstream.hostname == "127.0.0.1"
    attempts = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def forward(self):
            # Preserve Host and the request target so SigV4 stays unchanged.
            length = int(self.headers.get("Content-Length", "0"))
            if self.headers.get("Transfer-Encoding"):
                self.send_error(501, "fixture requires content-length")
                return
            body = self.rfile.read(length)
            connection = http.client.HTTPConnection(upstream.hostname, upstream.port, timeout=30)
            try:
                connection.request(self.command, self.path, body=body, headers=dict(self.headers))
                response = connection.getresponse()
                payload = response.read()
                complete = (self.command == "POST" and
                            "uploadId" in parse_qs(urlsplit(self.path).query))
                if complete:
                    attempts.append((response.status, payload))
                    self.close_connection = True
                    self.connection.shutdown(socket.SHUT_RDWR)
                    self.connection.close()
                    return
                self.send_response(response.status)
                for key, value in response.getheaders():
                    if key.lower() not in ("transfer-encoding", "connection", "content-length"):
                        self.send_header(key, value)
                content_length = response.getheader("Content-Length", "0") if self.command == "HEAD" else str(len(payload))
                self.send_header("Content-Length", content_length)
                self.end_headers()
                if self.command != "HEAD":
                    self.wfile.write(payload)
            finally:
                connection.close()

        do_GET = do_HEAD = do_PUT = do_POST = do_DELETE = forward

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}", attempts
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
