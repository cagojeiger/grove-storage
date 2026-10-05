import { useEffect, useRef, useState } from "react";
import {
  Alert,
  Button,
  FormControl,
  FormLabel,
  Link,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { useQueryClient } from "@tanstack/react-query";
import {
  ApiError,
  currentSession,
  identity,
  message,
  request,
} from "../api/http";
import { field } from "../api/identity";
import AuthTemplate from "../template/sign-in/SignIn";

let setupToken: string | null = null;
let revision = 0;
function captureLink() {
  if (!location.hash.startsWith("#set-password/")) return;
  setupToken = location.hash.slice("#set-password/".length);
  history.replaceState(
    null,
    "",
    `${location.pathname}${location.search}#set-password?link=${++revision}`,
  );
}
captureLink();
window.addEventListener("hashchange", captureLink);

export function Auth({ setup = false }: { setup?: boolean }) {
  const cache = useQueryClient();
  const lock = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [wait, setWait] = useState(0);
  const [token, setToken] = useState(setupToken);
  const [username, setUsername] = useState("");
  const [state, setState] = useState(
    setup ? (token ? "checking" : "unavailable") : "login",
  );
  useEffect(() => {
    if (wait <= 0) return;
    const timer = setTimeout(() => setWait(wait - 1), 1000);
    return () => clearTimeout(timer);
  }, [wait]);
  useEffect(() => {
    if (!setup || !token) return;
    let mounted = true;
    void request<{ username: string }>(`${identity}/password-setup/inspect`, {
      method: "POST",
      body: JSON.stringify({ token }),
    }).then(
      (info) => {
        if (mounted) {
          setUsername(info.username);
          setState("ready");
        }
      },
      () => {
        if (mounted) {
          setupToken = null;
          setToken(null);
          setState("unavailable");
        }
      },
    );
    return () => {
      mounted = false;
    };
  }, [setup, token]);
  return (
    <AuthTemplate>
      <Typography component="h1" variant="h4">
        {setup ? "Set password" : "Sign in"}
      </Typography>
      {state === "checking" && (
        <Typography role="status">Checking setup link...</Typography>
      )}
      {state === "unavailable" && (
        <Alert severity="error">
          This setup link is unavailable or expired. Ask an admin for a new
          link.
        </Alert>
      )}
      {state === "complete" && (
        <Alert severity="success">
          Password set. Sign in with your username and password.
        </Alert>
      )}
      {(state === "login" || state === "ready") && (
        <Stack
          component="form"
          spacing={2}
          onSubmit={(event) => {
            event.preventDefault();
            if (lock.current || wait) return;
            const form = event.currentTarget;
            const data = new FormData(form);
            const password = field(data, "password");
            if (setup && password !== field(data, "confirmation")) {
              setError("The passwords do not match.");
              return;
            }
            const loginName = setup ? username : field(data, "username").trim();
            for (const input of form.querySelectorAll<HTMLInputElement>(
              'input[type="password"]',
            ))
              input.value = "";
            lock.current = true;
            setBusy(true);
            setError("");
            void (async () => {
              try {
                if (setup) {
                  await request(`${identity}/password-setup`, {
                    method: "POST",
                    body: JSON.stringify({ token, password }),
                  });
                  setupToken = null;
                  setToken(null);
                  setState("complete");
                } else {
                  await request(`${identity}/session`, {
                    method: "POST",
                    body: JSON.stringify({
                      username: loginName,
                      password,
                    }),
                  });
                  cache.setQueryData(["session"], await currentSession());
                }
              } catch (failure) {
                if (
                  setup &&
                  failure instanceof ApiError &&
                  failure.status === 404
                ) {
                  setupToken = null;
                  setToken(null);
                  setState("unavailable");
                } else
                  setError(
                    failure instanceof ApiError && failure.status === 401
                      ? "Username or password is incorrect."
                      : message(failure),
                  );
                if (failure instanceof ApiError && failure.status === 429)
                  setWait(failure.retryAfter ?? 60);
              } finally {
                lock.current = false;
                setBusy(false);
              }
            })();
          }}
        >
          <FormControl>
            <FormLabel htmlFor="auth-username">Username</FormLabel>
            <TextField
              fullWidth
              id="auth-username"
              name="username"
              autoComplete="username"
              required
              disabled={busy}
              {...(setup
                ? {
                    value: username,
                    slotProps: { input: { readOnly: true } },
                  }
                : {})}
            />
          </FormControl>
          <FormControl>
            <FormLabel htmlFor="auth-password">
              {setup ? "New password" : "Password"}
            </FormLabel>
            <TextField
              fullWidth
              id="auth-password"
              name="password"
              type="password"
              autoComplete={setup ? "new-password" : "current-password"}
              required
              disabled={busy || wait > 0}
              slotProps={{
                htmlInput: setup ? { minLength: 15, maxLength: 128 } : {},
              }}
            />
          </FormControl>
          {setup && (
            <FormControl>
              <FormLabel htmlFor="auth-confirmation">
                Confirm password
              </FormLabel>
              <TextField
                fullWidth
                id="auth-confirmation"
                name="confirmation"
                type="password"
                autoComplete="new-password"
                required
                disabled={busy}
                slotProps={{ htmlInput: { minLength: 15, maxLength: 128 } }}
              />
            </FormControl>
          )}
          {error && <Alert severity="error">{error}</Alert>}
          <Button variant="contained" type="submit" disabled={busy || wait > 0}>
            {wait
              ? `Retry in ${wait}s`
              : busy
                ? "Please wait..."
                : setup
                  ? "Set password"
                  : "Sign in"}
          </Button>
        </Stack>
      )}
      {(state === "complete" || state === "unavailable") && (
        <Link href="#">Sign in</Link>
      )}
    </AuthTemplate>
  );
}
