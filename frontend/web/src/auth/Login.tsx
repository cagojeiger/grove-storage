import {
  Alert,
  Container,
  Stack,
  Typography,
  TextField,
  Button,
} from "@mui/material";

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
    <Container component="main" maxWidth="xs" sx={{ py: { xs: 4, sm: 8 } }}>
      <Stack spacing={3}>
        <Typography component="h1" variant="h5">
          Sign in
        </Typography>
        <Stack
          component="form"
          spacing={3}
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
          {error && <Alert severity="error">{error}</Alert>}
          <Button
            variant="contained"
            type="submit"
            endIcon={<ArrowRight size={16} />}
            disabled={pending || !username.trim() || !password || wait > 0}
          >
            {pending ? "Signing in" : wait ? `Retry in ${wait}s` : "Sign in"}
          </Button>
        </Stack>
        <Typography variant="body2" color="text.secondary">
          Lost access? Contact the server operator for account recovery.
        </Typography>
      </Stack>
    </Container>
  );
}
