import { TextField, Button } from "@mui/material";

import { FormEvent, useEffect, useState } from "react";
import { ArrowRight } from "lucide-react";
import {
  identity,
  ApiError,
  currentSession,
  message,
  request,
  Session,
} from "../api/http";

export function Login({ onLogin }: { onLogin: (session: Session) => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [wait, setWait] = useState(0);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!wait) return;
    const timer = setTimeout(() => setWait(wait - 1), 1000);
    return () => clearTimeout(timer);
  }, [wait]);
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (pending || wait) return;
    const loginName = username.trim();
    const secret = password;
    setPassword("");
    setPending(true);
    setError("");
    // Keep credentials out of query/mutation caches and browser storage.
    try {
      await request(`${identity}/session`, {
        method: "POST",
        body: JSON.stringify({ username: loginName, password: secret }),
      });
      onLogin(await currentSession());
    } catch (failure) {
      setError(message(failure));
      if (failure instanceof ApiError && failure.status === 429)
        setWait(failure.retryAfter ?? 60);
    } finally {
      setPending(false);
    }
  }
  return (
    <main className="login">
      <img
        className="login-logo"
        src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
        alt=""
      />
      <h1>Grove Storage</h1>
      <h2>Sign in</h2>
      <form
        onSubmit={(event) => {
          void submit(event);
        }}
      >
        <TextField
          label="Username"
          id="username"
          autoComplete="username"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          required
          disabled={pending}
          slotProps={{ htmlInput: { spellCheck: false } }}
        />
        <TextField
          label="Password"
          id="password"
          type="password"
          autoComplete="current-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          required
          disabled={pending}
          slotProps={{ htmlInput: { spellCheck: false } }}
        />
        {error && <p role="alert">{error}</p>}
        <Button
          variant="contained"
          className="primary"
          type="submit"
          disabled={pending || !username.trim() || !password || wait > 0}
        >
          {pending ? "Signing in" : wait ? `Retry in ${wait}s` : "Sign in"}
          <ArrowRight size={16} aria-hidden="true" />
        </Button>
      </form>
      <p className="muted">
        Lost access? Contact the server operator for account recovery.
      </p>
    </main>
  );
}
