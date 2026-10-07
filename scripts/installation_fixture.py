"""Owned, disposable processes and databases for fresh-installation verification."""

from contextlib import contextmanager
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request
import uuid

from s3_backend_fixture import docker

ROOT = Path(__file__).resolve().parent.parent
SERVER = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve() / "debug/grove-storage"
CONSOLE_ORIGIN = "https://console.test"


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


class InstallationFixture:
    def __init__(self, container, directory, db_port):
        self.container = container
        self.directory = Path(directory)
        self.endpoint = f"http://127.0.0.1:{free_port()}"
        self.db_port = db_port
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def environment(self, database="grove"):
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(("GROVE_", "AWS_")) and k != "DATABASE_URL"}
        env.update(
            GROVE_DATABASE_URL=f"postgres://grove:grove@127.0.0.1:{self.db_port}/{database}",
            GROVE_ENC_ROOT_SECRET="installation-fixture-encryption-secret-32bytes",
            GROVE_BIND=self.endpoint.removeprefix("http://"),
            GROVE_PUBLIC_URL=self.endpoint, GROVE_CONSOLE_ORIGIN=CONSOLE_ORIGIN,
            GROVE_RECONCILER_INTERVAL_SECS="3600", GROVE_LOG_FORMAT="json",
        )
        return env

    def sql(self, statement, database="grove"):
        result = subprocess.run(
            ["docker", "exec", "-i", self.container, "psql", "-X", "-qAt",
             "-U", "grove", "-d", database, "-v", "ON_ERROR_STOP=1"],
            input=statement, text=True, capture_output=True, check=True, timeout=30,
        )
        return result.stdout.strip()

    def snapshot(self, tables, *, database="grove"):
        result = {}
        for table in tables:
            identifier = ".".join('"' + part.replace('"', '""') + '"' for part in table.split("."))
            result[table] = json.loads(self.sql(
                f"SELECT coalesce(jsonb_agg(row ORDER BY row::text),'[]') FROM "
                f"(SELECT to_jsonb(t) AS row FROM {identifier} t) s;", database))
        return result

    def request(self, method, path, body=None, token=None):
        headers = {"Content-Type": "application/json"}
        if token is not None:
            headers["Authorization"] = "Bearer " + token
        request = urllib.request.Request(
            self.endpoint + path, method=method, headers=headers,
            data=None if body is None else json.dumps(body).encode(),
        )
        with self.opener.open(request, timeout=10) as response:
            content = response.read()
            return json.loads(content) if content else None

    @contextmanager
    def server(self, binary, *, database="grove"):
        with tempfile.TemporaryFile() as log:
            process = subprocess.Popen([str(binary)], cwd=self.directory,
                                       env=self.environment(database), stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 30
                while True:
                    if process.poll() is not None:
                        log.seek(0, os.SEEK_END)
                        log.seek(max(log.tell() - 8192, 0))
                        details = log.read().decode(errors="replace")
                        raise RuntimeError(f"installation server exited before readiness:\n{details}")
                    try:
                        if self.request("GET", "/readyz") == {"status": "ready"}:
                            break
                    except OSError:
                        pass
                    if time.monotonic() >= deadline:
                        raise RuntimeError("installation server readiness timeout")
                    time.sleep(0.1)
                yield
            finally:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)

    def backup(self):
        path = self.directory / "installation.dump"
        with path.open("xb") as output:
            path.chmod(0o600)
            subprocess.run(["docker", "exec", self.container, "pg_dump", "-U", "grove",
                            "-d", "grove", "--format=custom"], stdout=output,
                           check=True, timeout=60)
        return path

    def restore(self, path):
        # The destination is always a new DB inside our own container, never an input URL.
        docker("exec", self.container, "createdb", "-U", "grove", "restored")
        with path.open("rb") as source:
            subprocess.run(["docker", "exec", "-i", self.container, "pg_restore",
                            "-U", "grove", "-d", "restored", "--exit-on-error",
                            "--single-transaction"], stdin=source, check=True, timeout=60)


@contextmanager
def installation():
    name = "grove-installation-" + uuid.uuid4().hex[:12]
    try:
        docker("run", "--rm", "-d", "--name", name, "-p", "127.0.0.1::5432",
               "-e", "POSTGRES_USER=grove", "-e", "POSTGRES_PASSWORD=grove",
               "-e", "POSTGRES_DB=grove", "postgres:17-alpine")
        deadline = time.monotonic() + 30
        # The initialization-only PostgreSQL server accepts Unix sockets, not TCP.
        while subprocess.run(["docker", "exec", name, "pg_isready", "-h", "127.0.0.1", "-U", "grove"],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5).returncode:
            if time.monotonic() >= deadline:
                raise RuntimeError("installation PostgreSQL readiness timeout")
            time.sleep(0.2)
        port = docker("port", name, "5432").rsplit(":", 1)[1]
        with tempfile.TemporaryDirectory(prefix="grove-installation-") as directory:
            yield InstallationFixture(name, directory, port)
    finally:
        subprocess.run(["docker", "rm", "-f", "-v", name], check=True, timeout=30,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print("PASS installation database, backup, and processes cleaned up")
