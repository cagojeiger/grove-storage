"""Loopback-only OIDC test issuer; never a production authentication provider."""

import base64
from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import secrets
from threading import Thread
import time
from urllib.parse import parse_qs, urlencode, urlsplit

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding, rsa


def b64(value):
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


@contextmanager
def oidc_fixture(redirect_uri):
    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    public = key.public_key().public_numbers()
    jwk = {"kty": "RSA", "kid": "local-browser-test", "use": "sig", "alg": "RS256",
           "n": b64(public.n.to_bytes((public.n.bit_length() + 7) // 8, "big")),
           "e": b64(public.e.to_bytes(3, "big"))}
    codes, access_tokens = {}, set()
    events = []
    identity = {"sub": "grove-local-browser-test", "email": "browser@example.test",
                "email_verified": True, "name": "Grove Browser Test"}

    def token(claims):
        header = b64(json.dumps({"alg": "RS256", "kid": jwk["kid"]}).encode())
        payload = b64(json.dumps(claims).encode())
        data = (header + "." + payload).encode()
        return data.decode() + "." + b64(key.sign(data, padding.PKCS1v15(), hashes.SHA256()))

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def send_json(self, value, status=200):
            data = json.dumps(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            path = urlsplit(self.path)
            if path.path == "/.well-known/openid-configuration":
                self.send_json({"issuer": issuer, "authorization_endpoint": issuer + "/authorize",
                    "token_endpoint": issuer + "/token", "userinfo_endpoint": issuer + "/userinfo",
                    "jwks_uri": issuer + "/keys", "response_types_supported": ["code"],
                    "subject_types_supported": ["public"], "id_token_signing_alg_values_supported": ["RS256"],
                    "token_endpoint_auth_methods_supported": ["none"],
                    "code_challenge_methods_supported": ["S256"]})
            elif path.path == "/keys":
                self.send_json({"keys": [jwk]})
            elif path.path == "/authorize":
                args = {k: v[0] for k, v in parse_qs(path.query).items()}
                if (args.get("client_id") != "notegate-web" or args.get("redirect_uri") != redirect_uri
                        or args.get("code_challenge_method") != "S256" or args.get("response_type") != "code"
                        or not all(args.get(k) for k in ("state", "nonce", "code_challenge"))):
                    return self.send_json({"error": "invalid_request"}, 400)
                code = secrets.token_urlsafe(24)
                codes[code] = (args, time.time())
                events.append("authorize")
                self.send_response(302)
                self.send_header("Location", redirect_uri + "?" + urlencode({"code": code, "state": args["state"]}))
                self.end_headers()
            elif path.path == "/userinfo" and self.headers.get("Authorization", "").removeprefix("Bearer ") in access_tokens:
                events.append("userinfo")
                self.send_json(identity)
            else:
                self.send_json({"error": "not_found"}, 404)

        def do_POST(self):
            if self.path != "/token":
                return self.send_json({"error": "not_found"}, 404)
            args = {k: v[0] for k, v in parse_qs(self.rfile.read(int(self.headers.get("Content-Length", "0"))).decode()).items()}
            grant = codes.pop(args.get("code"), None)
            if (not grant or time.time() - grant[1] > 60 or args.get("grant_type") != "authorization_code"
                    or args.get("client_id") != "notegate-web" or args.get("redirect_uri") != redirect_uri
                    or b64(hashlib.sha256(args.get("code_verifier", "").encode()).digest()) != grant[0]["code_challenge"]):
                return self.send_json({"error": "invalid_grant"}, 400)
            access, refresh = secrets.token_urlsafe(24), secrets.token_urlsafe(24)
            access_tokens.add(access)
            now = int(time.time())
            events.append("token_pkce_verified")
            self.send_json({"access_token": access, "refresh_token": refresh, "token_type": "Bearer", "expires_in": 3600,
                "id_token": token({**identity, "iss": issuer, "aud": "notegate-web", "iat": now,
                    "exp": now + 3600, "nonce": grant[0]["nonce"],
                    "at_hash": b64(hashlib.sha256(access.encode()).digest()[:16])})})

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    issuer = f"http://127.0.0.1:{server.server_port}"
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield issuer, events
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
