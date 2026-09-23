# Grove Storage Console

| Implemented | Follow-up |
|---|---|
| Admin cookie login/logout, session restore and 401 handling | Storage/client mutation screens |
| API readiness, client count, per-storage usage | Usage history and client detail |
| System/light/dark, mobile/tablet/desktop | Production static hosting and TLS ingress |

## Isolated Local Console

From the repository root, build `filegate` and `gscli` with `cargo build --bin filegate --bin gscli --locked`.
On macOS use `DEVELOPER_DIR=/Library/Developer/CommandLineTools` if required by the Rust linker.

```sh
cd frontend/web
npm ci
npx playwright install chromium
npm run build
npm test
cd ../..
python3 -B -u scripts/e2e-console.py
python3 -B -u scripts/e2e-console.py --serve
```

The fixture needs Docker, Python 3, Node 22+ and OpenSSL. It creates a disposable
PostgreSQL database, API and HTTPS Vite server. `--serve` prints the URL and a local
mode-0600 token file. The certificate is self-signed and scoped to this local fixture;
the browser may show a trust warning. Ctrl-C or SIGTERM cleans up the fixture.
No production endpoint or credentials are used. The initial overview is empty.

## Existing Development API

```sh
GROVE_DEV_API=http://127.0.0.1:8080 \
GROVE_DEV_TLS_KEY=/absolute/path/key.pem \
GROVE_DEV_TLS_CERT=/absolute/path/cert.pem npm run dev
```

Set the API's `FILEGATE_CONSOLE_ORIGIN` to the exact HTTPS Vite origin and initialize
a DB admin token using `filegate admin init`. Legacy environment tokens do not log
into the console. The default UI port is 5173; an occupied port fails instead of
silently changing the allowed origin. Override with `-- --port PORT` and update the origin.

The [Vite HTTPS/proxy configuration](https://vite.dev/config/server-options) retains
the browser Origin. API routes and `/readyz` are proxied; browser requests use
same-origin cookies. Token/provider secrets never belong in `VITE_*` variables.
For release hosting, mount `dist` at `/api/admin/console/` and forward existing API
routes unchanged. This commit does not alter the backend image or production ingress.

## Verification

| Test | Boundary |
|---|---|
| `tests/api.spec.ts` | Transport options, cancellation signal, error sanitization, formatting |
| `tests/console.spec.ts` | Mock API state/error handling, 320/390/768/1024/1440px, themes, screenshots |
| `tests/live.mjs` via Python fixture | Real HTTPS cookie attributes, CSRF, reload/logout, storage usage, expiry and token revocation |

Expiry is injected into the isolated database; the test does not wait eight hours.
Real storage registration in the browser fixture uses a temporary filesystem root.
