"""Loopback HTTPS page/proxy with a trusted fixture certificate and unmodified Host/path/query."""

from contextlib import contextmanager
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading
from urllib.parse import urlsplit


@contextmanager
def https_fixture(upstream=None):
    target = urlsplit(upstream) if upstream else None
    if target and (target.scheme != "http" or target.hostname != "127.0.0.1"):
        raise ValueError("HTTPS contract proxy only accepts loopback HTTP fixtures")
    with tempfile.TemporaryDirectory(prefix="grove-s3-tls-") as directory:
        key, cert = Path(directory) / "key.pem", Path(directory) / "cert.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout", str(key), "-out", str(cert), "-days", "1", "-subj", "/CN=127.0.0.1",
            "-addext", "subjectAltName=IP:127.0.0.1,DNS:localhost"],
            check=True, capture_output=True, timeout=30)

        class Handler(BaseHTTPRequestHandler):
            def forward(self):
                if not target:
                    body = b"<!doctype html><title>S3 browser contract fixture</title>"
                    self.send_response(200)
                    self.send_header("Content-Type", "text/html")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    return
                connection = HTTPConnection(target.hostname, target.port, timeout=30)
                try:
                    body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                    headers = {k: v for k, v in self.headers.items()
                               if k.lower() not in {"connection", "transfer-encoding"}}
                    connection.request(self.command, self.path, body=body, headers=headers)
                    response = connection.getresponse()
                    payload = response.read()
                    self.send_response(response.status)
                    for name, value in response.getheaders():
                        if name.lower() not in {"connection", "transfer-encoding", "content-length"}:
                            self.send_header(name, value)
                    length = response.getheader("Content-Length") if self.command == "HEAD" else str(len(payload))
                    if length is not None:
                        self.send_header("Content-Length", length)
                    self.end_headers()
                    if self.command != "HEAD":
                        self.wfile.write(payload)
                finally:
                    connection.close()

            do_GET = do_PUT = do_POST = do_DELETE = do_HEAD = do_OPTIONS = forward

            def log_message(self, *_args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield f"https://127.0.0.1:{server.server_port}", cert
        finally:
            server.shutdown()
            thread.join(timeout=5)
            server.server_close()
