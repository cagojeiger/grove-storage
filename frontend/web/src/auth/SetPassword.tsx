import {
  Alert,
  Container,
  Link,
  Stack,
  Typography,
  TextField,
  Button,
} from "@mui/material";

import { FormEvent, useEffect, useState } from "react";
import { identity, ApiError, message, request } from "../api/http";
import { field } from "../api/identity";

let setupToken: string | null = null;
let setupLinkVersion = 0;
function captureSetupToken() {
  const prefix = "#set-password/";
  if (!window.location.hash.startsWith(prefix)) return;
  setupToken = window.location.hash.slice(prefix.length);
  window.history.replaceState(
    null,
    "",
    `${window.location.pathname}${window.location.search}#set-password?link=${++setupLinkVersion}`,
  );
}
captureSetupToken();
window.addEventListener("hashchange", captureSetupToken);

function takeToken() {
  return setupToken;
}

type SetupInfo = { username: string; expires_at: string };

export function SetPassword() {
  const [token, setToken] = useState(takeToken);
  const [info, setInfo] = useState<SetupInfo | null>(null);
  const [status, setStatus] = useState<
    "loading" | "ready" | "unavailable" | "complete"
  >(token ? "loading" : "unavailable");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!token) return;
    let active = true;
    void request<SetupInfo>(`${identity}/password-setup/inspect`, {
      method: "POST",
      body: JSON.stringify({ token }),
    })
      .then((value) => {
        if (active) {
          setInfo(value);
          setStatus("ready");
        }
      })
      .catch(() => {
        if (active) {
          setupToken = null;
          setToken(null);
          setStatus("unavailable");
        }
      });
    return () => {
      active = false;
    };
  }, [token]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!token || pending) return;
    const form = event.currentTarget;
    const data = new FormData(form);
    const password = field(data, "password");
    if (password !== field(data, "confirmation")) {
      setError("The passwords do not match.");
      return;
    }
    form.reset();
    setPending(true);
    setError("");
    try {
      await request(`${identity}/password-setup`, {
        method: "POST",
        body: JSON.stringify({ token, password }),
      });
      setupToken = null;
      setToken(null);
      setStatus("complete");
    } catch (failure) {
      if (failure instanceof ApiError && failure.status === 404) {
        setupToken = null;
        setToken(null);
        setStatus("unavailable");
      } else {
        setError(
          failure instanceof ApiError && failure.status === 400
            ? "Choose a password of 15 to 128 characters."
            : message(failure),
        );
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <Container component="main" maxWidth="xs" sx={{ py: { xs: 4, sm: 8 } }}>
      <Stack spacing={3}>
        <Typography component="h1" variant="h5">
          Set password
        </Typography>
        {status === "loading" && (
          <Typography role="status">Checking setup link...</Typography>
        )}
        {status === "unavailable" && (
          <Alert severity="error">
            This setup link is unavailable or expired. Ask an admin for a new
            link.
          </Alert>
        )}
        {status === "complete" && (
          <Alert severity="success" role="status">
            Password set. Sign in with your username and password.
          </Alert>
        )}
        {status === "ready" && info && (
          <Stack
            component="form"
            spacing={3}
            onSubmit={(event) => {
              void submit(event);
            }}
          >
            <TextField
              label="Username"
              id="setup-username"
              value={info.username}
              autoComplete="username"
              slotProps={{ input: { readOnly: true } }}
            />
            <TextField
              label="New password"
              id="setup-password"
              name="password"
              type="password"
              autoComplete="new-password"
              required
              disabled={pending}
              slotProps={{ htmlInput: { minLength: 15, maxLength: 128 } }}
            />
            <TextField
              label="Confirm password"
              id="setup-confirmation"
              name="confirmation"
              type="password"
              autoComplete="new-password"
              required
              disabled={pending}
              slotProps={{ htmlInput: { minLength: 15, maxLength: 128 } }}
            />
            {error && <Alert severity="error">{error}</Alert>}
            <Button variant="contained" type="submit" disabled={pending}>
              {pending ? "Setting password" : "Set password"}
            </Button>
          </Stack>
        )}
        {(status === "complete" || status === "unavailable") && (
          <Link href="#">Sign in</Link>
        )}
      </Stack>
    </Container>
  );
}
