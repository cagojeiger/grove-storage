#!/usr/bin/env python3
"""Exercise the packaged console and API in a hardened image with a fresh DB."""

import argparse
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import runpy
import secrets
import ssl
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid

from image_s3_contract import check as check_s3

ROOT = Path(__file__).resolve().parent.parent
run_tty = runpy.run_path(str(ROOT / "scripts/e2e-password-account.py"))["run_tty"]


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True, timeout=90).strip()


def main(image, report_dir):
    name = "grove-image-" + uuid.uuid4().hex[:12]
    database, server = name + "-db", name + "-server"
    report_dir.mkdir(parents=True, exist_ok=True)
    inspection = json.loads(docker("image", "inspect", image))[0]
    assert inspection["Config"]["User"] == "10001:10001"
    (report_dir / "image.json").write_text(json.dumps({
        "image": image, "id": inspection["Id"], "architecture": inspection["Architecture"],
    }, indent=2))
    password = "fixture-" + secrets.token_urlsafe(32)
    docker("network", "create", name)
    proxy = None
    thread = None
    try:
        with tempfile.TemporaryDirectory(prefix="grove-image-") as directory:
            directory = Path(directory)
            docker("run", "--rm", "-d", "--name", database, "--network", name,
                   "-e", "POSTGRES_USER=grove", "-e", "POSTGRES_PASSWORD=grove",
                   "-e", "POSTGRES_DB=grove", "postgres:17-alpine")
            deadline = time.monotonic() + 30
            while subprocess.run(["docker", "exec", database, "pg_isready", "-h", "127.0.0.1", "-U", "grove", "-d", "grove"],
                                 capture_output=True, timeout=5).returncode:
                if time.monotonic() > deadline:
                    raise RuntimeError("image fixture database timeout")
                time.sleep(.2)

            # This proxy terminates fixture TLS only; production TLS belongs to ingress.
            class Proxy(BaseHTTPRequestHandler):
                def forward(self):
                    connection = HTTPConnection("127.0.0.1", backend_port, timeout=15)
                    try:
                        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                        headers = {key: value for key, value in self.headers.items()
                                   if key.lower() not in {"connection", "transfer-encoding"}}
                        connection.request(self.command, self.path, body=body, headers=headers)
                        response = connection.getresponse()
                        payload = response.read()
                        self.send_response(response.status)
                        for key, value in response.getheaders():
                            if key.lower() not in {"connection", "content-length", "transfer-encoding"}:
                                self.send_header(key, value)
                        self.send_header("Content-Length", str(len(payload)))
                        self.end_headers()
                        if self.command != "HEAD":
                            self.wfile.write(payload)
                    except (BrokenPipeError, ConnectionResetError):
                        pass
                    finally:
                        connection.close()

                do_GET = do_POST = do_PATCH = do_DELETE = do_HEAD = forward

                def log_message(self, *_args):
                    pass

            proxy = ThreadingHTTPServer(("127.0.0.1", 0), Proxy)
            origin = f"https://127.0.0.1:{proxy.server_port}"
            env_file = directory / "env"
            env_file.touch(mode=0o600)
            env_file.write_text("\n".join([
                f"GROVE_DATABASE_URL=postgres://grove:grove@{database}:5432/grove",
                "GROVE_ENC_ROOT_SECRET=" + secrets.token_urlsafe(48),
                f"GROVE_CONSOLE_ORIGIN={origin}", "GROVE_LOG_FORMAT=json",
            ]) + "\n")
            hardened = ["--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
                        "--tmpfs", "/tmp:rw,noexec,nosuid,size=64m"]
            conflict = subprocess.run(["docker", "run", "--rm", "--network", name,
                "--env-file", str(env_file), "-e", f"GROVE_PUBLIC_URL={origin}",
                *hardened, image], capture_output=True, text=True, timeout=15)
            assert conflict.returncode != 0
            assert "GROVE_PUBLIC_URL must not use the console host" in conflict.stderr
            print("PASS actual image rejects conflicting console/object origins before account initialization")
            run_tty(["docker", "run", "--rm", "-it", "--network", name, "--env-file", str(env_file),
                     *hardened, image, "account", "init", "owner", "Image test owner"], password)
            docker("run", "-d", "--name", server, "--network", name,
                   "--env-file", str(env_file), "-p", "127.0.0.1::8080", *hardened, image)
            backend_port = int(docker("port", server, "8080").rsplit(":", 1)[1])
            endpoint = f"http://127.0.0.1:{backend_port}"
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            deadline = time.monotonic() + 30
            while True:
                try:
                    with opener.open(endpoint + "/readyz", timeout=2) as response:
                        assert json.load(response) == {"status": "ready"}
                    break
                except OSError:
                    if time.monotonic() > deadline:
                        raise RuntimeError("packaged image readiness timeout")
                    time.sleep(.2)
            for path in ["/api/admin/console/", "/api/admin/identity/v1/session"]:
                try:
                    opener.open(endpoint + path, timeout=5)
                except urllib.error.HTTPError as error:
                    assert error.code == 404
                else:
                    raise AssertionError("object host exposed console")
            key, cert = directory / "key.pem", directory / "cert.pem"
            subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                            "-keyout", str(key), "-out", str(cert), "-days", "1",
                            "-subj", "/CN=127.0.0.1"], check=True, capture_output=True, timeout=30)
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(cert, key)
            proxy.socket = context.wrap_socket(proxy.socket, server_side=True)
            thread = threading.Thread(target=proxy.serve_forever, daemon=True)
            thread.start()
            subprocess.run(["node", "tests/image-runtime.mjs"], cwd=ROOT / "frontend/web",
                           input=json.dumps({"origin": origin, "password": password,
                                             "reportDir": str(report_dir.resolve())}),
                           text=True, check=True, timeout=150)
            check_s3(endpoint, origin, password, name, report_dir)
            docker("stop", "--time", "15", server)
            assert docker("inspect", server, "--format", "{{.State.ExitCode}}") == "0"
            print("PASS actual image: fresh DB, hidden TTY bootstrap, non-root/read-only runtime, HTTPS console, host isolation, SIGTERM")
    finally:
        if thread:
            proxy.shutdown()
            thread.join(timeout=5)
        if proxy:
            proxy.server_close()
        for container in (server, database):
            subprocess.run(["docker", "rm", "-f", container], capture_output=True, timeout=30)
        subprocess.run(["docker", "network", "rm", name], capture_output=True, timeout=30)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("image")
    parser.add_argument("--report-dir", type=Path, default=ROOT / "reports/image-runtime")
    args = parser.parse_args()
    main(args.image, args.report_dir)
