# Console Preview Tests

This suite checks the interactive preview in `../filegate-console-management.html`.
It uses an isolated iframe and fake data; it does not call FileGate or S3.

```sh
cd output/console-tests
npm ci
npx playwright install chromium
npm test
```

| File | Coverage |
|---|---|
| storage.test.cjs | Registration, edit, validation, deletion guards, confirmation |
| clients.test.cjs | Client lifecycle, key validation, one-time secret, revocation |
| layout.test.cjs | Mobile/tablet/desktop, themes, login, dialog dismissal |

The backend authentication and database lifecycle tests remain in the Rust
workspace. After the real frontend is connected, add HTTP integration tests for
401/409, expired sessions, in-flight deletion races, timeouts and unknown mutation
outcomes. A preview test passing is not evidence of S3 connectivity or API parity.
