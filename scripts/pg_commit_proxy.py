"""Drop a selected COMMIT acknowledgment after PostgreSQL confirms it, on loopback only."""

import socket
import socketserver
import struct
import threading
from urllib.parse import urlsplit, urlunsplit


def exact(stream, size):
    data = b""
    while len(data) < size:
        chunk = stream.recv(size - len(data))
        if not chunk:
            raise EOFError()
        data += chunk
    return data


def frame(stream):
    kind = exact(stream, 1)
    length = exact(stream, 4)
    size = struct.unpack("!I", length)[0]
    if not 4 <= size <= 16 * 1024 * 1024:
        raise ValueError("invalid PostgreSQL fixture frame")
    payload = exact(stream, size - 4)
    return kind, payload, kind + length + payload


class CommitProxy:
    def __init__(self):
        self.target = None
        self.hits = 0
        self.upstream = None
        proxy = self

        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                remote = socket.create_connection(proxy.upstream, timeout=5)
                remote.settimeout(None)
                statements = {}
                selected = False

                def relay(source, destination, incoming):
                    nonlocal selected
                    try:
                        while True:
                            kind, payload, raw = frame(source)
                            if incoming and kind == b"P":
                                name, query, _ = payload.split(b"\0", 2)
                                statements[name] = query
                            if incoming and kind == b"B":
                                _, name, bindings = payload.split(b"\0", 2)
                                query = statements.get(name, b"").upper()
                                target = proxy.target
                                if (target and b"DELETE FROM UPLOADS" in query and
                                        (target.bytes in bindings or str(target).encode() in bindings)):
                                    selected = True
                            if not incoming and kind == b"C":
                                if payload == b"COMMIT\0" and selected and proxy.target:
                                    proxy.hits += 1
                                    proxy.target = None
                                    return
                                if payload in (b"COMMIT\0", b"ROLLBACK\0"):
                                    selected = False
                            destination.sendall(raw)
                    except (EOFError, OSError):
                        pass
                    finally:
                        for stream in (self.request, remote):
                            try:
                                stream.shutdown(socket.SHUT_RDWR)
                            except OSError:
                                pass

                try:
                    length = exact(self.request, 4)
                    size = struct.unpack("!I", length)[0]
                    if not 8 <= size <= 65536:
                        raise ValueError("invalid PostgreSQL fixture startup")
                    remote.sendall(length + exact(self.request, size - 4))
                    reader = threading.Thread(target=relay, args=(self.request, remote, True), daemon=True)
                    reader.start()
                    relay(remote, self.request, False)
                    reader.join(timeout=5)
                except (EOFError, OSError):
                    pass
                finally:
                    remote.close()

        self.server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def route(self, url):
        address = urlsplit(url)
        if address.scheme not in ("postgres", "postgresql") or address.hostname != "127.0.0.1":
            raise ValueError("COMMIT fault fixture requires a loopback PostgreSQL URL")
        self.upstream = (address.hostname, address.port or 5432)
        credentials = address.netloc.rsplit("@", 1)[0]
        return urlunsplit((address.scheme,
            f"{credentials}@127.0.0.1:{self.server.server_address[1]}",
            address.path, "sslmode=disable", ""))

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_args):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
