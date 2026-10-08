import socket
import struct
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from pg_commit_proxy import CommitProxy, frame


class CommitProxyTests(unittest.TestCase):
    def test_reads_fragmented_frames(self):
        a, b = socket.socketpair()
        try:
            payload = b"COMMIT\0"
            raw = b"C"+struct.pack("!I", len(payload)+4)+payload
            a.sendall(raw[:3])
            a.sendall(raw[3:])
            self.assertEqual(frame(b), (b"C", payload, raw))
        finally:
            a.close()
            b.close()

    def test_rejects_invalid_frame_lengths(self):
        a, b = socket.socketpair()
        try:
            a.sendall(b"C"+struct.pack("!I", 3))
            with self.assertRaises(ValueError):
                frame(b)
        finally:
            a.close()
            b.close()

    def test_route_is_loopback_only(self):
        with CommitProxy() as proxy:
            with self.assertRaises(ValueError):
                proxy.route("postgres://user:secret@production.example/db")
            route = proxy.route("postgres://grove:fixture@127.0.0.1:5432/grove")
            self.assertEqual(proxy.upstream, ("127.0.0.1", 5432))
            self.assertIn("sslmode=disable", route)


if __name__ == "__main__":
    unittest.main()
