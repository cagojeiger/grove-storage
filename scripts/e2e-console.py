#!/usr/bin/env python3
"""Real HTTPS browser session checks against a disposable PostgreSQL/API."""

import argparse
import json
import os
from pathlib import Path
import runpy
import socket
import signal
import ssl
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
HARNESS = runpy.run_path(str(ROOT / "scripts/e2e-cli.py"))


def check(endpoint, directory, database, origin, serve, minio):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    deadline = time.monotonic() + 30
    while True:
        try:
            with opener.open(endpoint + '/readyz', timeout=2) as response:
                if response.status == 200:
                    break
        except OSError:
            pass
        if time.monotonic() > deadline:
            raise RuntimeError('API readiness timeout')
        time.sleep(.2)
    headers = {'Origin': origin, 'X-Grove-CSRF': '1', 'Content-Type': 'application/json'}
    def identity(path, body):
        request = urllib.request.Request(endpoint + '/api/admin/identity/v1' + path,
                                         headers=headers, data=json.dumps(body).encode())
        with opener.open(request, timeout=5) as response:
            cookie = response.headers.get('Set-Cookie')
            if cookie:
                headers['Cookie'] = cookie.split(';', 1)[0]
            return json.loads(response.read())
    if serve:
        identity('/master/session', {'token': HARNESS['MASTER_TOKEN']})
        credential = identity('/master/bootstrap', {'display_name': 'Console test owner'})
        token = credential['token']
    key, cert = Path(directory) / 'key.pem', Path(directory) / 'cert.pem'
    subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes',
                    '-keyout', str(key), '-out', str(cert), '-days', '1',
                    '-subj', '/CN=127.0.0.1'], check=True, capture_output=True, timeout=30)
    dev_env = dict(os.environ, GROVE_DEV_API=endpoint, GROVE_DEV_TLS_KEY=str(key), GROVE_DEV_TLS_CERT=str(cert))
    with (Path(directory) / 'vite.log').open('w') as log:
        vite = subprocess.Popen(['node', 'node_modules/vite/bin/vite.js', '--port', origin.rsplit(':', 1)[1]],
                                cwd=ROOT / 'frontend/web', env=dev_env, stdout=log, stderr=log)
        try:
            https = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                urllib.request.HTTPSHandler(context=ssl._create_unverified_context()))
            deadline = time.monotonic() + 30
            while True:
                try:
                    with https.open(origin + '/api/admin/console/', timeout=2):
                        break
                except OSError:
                    if vite.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError('HTTPS Vite readiness timeout')
                    time.sleep(.2)
            if serve:
                token_file = Path(directory) / 'user-token'
                fd = os.open(token_file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                with os.fdopen(fd, 'w') as output:
                    output.write(token)
                print('Console:', origin + '/api/admin/console/', flush=True)
                print('Local disposable User token:', token_file, flush=True)
                while vite.poll() is None:
                    time.sleep(1)
            else:
                subprocess.run(['node', 'tests/live.mjs'], cwd=ROOT / 'frontend/web',
                               input=json.dumps({'origin': origin, 'masterToken': HARNESS['MASTER_TOKEN'],
                                                 'database': database, 'endpoint': endpoint,
                                                 'minio': minio.spec}), text=True,
                               check=True, timeout=150)
        finally:
            vite.terminate()
            try:
                vite.wait(timeout=10)
            except subprocess.TimeoutExpired:
                vite.kill(); vite.wait(timeout=5)


if __name__ == '__main__':
    def stop(_signal, _frame):
        raise SystemExit(0)
    signal.signal(signal.SIGTERM, stop)
    parser = argparse.ArgumentParser()
    parser.add_argument('--serve', action='store_true')
    parser.add_argument('--with-minio', action='store_true')
    args = parser.parse_args()
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    origin = f'https://127.0.0.1:{port}'
    from s3_backend_fixture import minio_backend
    with minio_backend() as minio:
        HARNESS['main'](lambda endpoint, directory, database: check(endpoint, directory, database, origin, args.serve, minio),
                        with_database=True, console_origin=origin, management=True)
