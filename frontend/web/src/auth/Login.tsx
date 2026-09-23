import { FormEvent, useEffect, useState } from "react";
import { ArrowRight } from "lucide-react";
import { admin, ApiError, message, request, Session } from "../api/http";

export function Login({ onLogin }: { onLogin: (session: Session) => void }) {
  const [token, setToken] = useState("");
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
    const value = token.trim();
    setToken("");
    setPending(true);
    setError("");
    // Keep credentials out of query/mutation caches and browser storage.
    try {
      onLogin(
        await request<Session>(`${admin}/session`, {
          method: "POST",
          body: JSON.stringify({ token: value }),
        }),
      );
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
      <h2>관리자 로그인</h2>
      <form onSubmit={submit}>
        <label htmlFor="token">관리자 토큰</label>
        <input
          id="token"
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={token}
          onChange={(event) => setToken(event.target.value)}
          required
          disabled={pending}
        />
        {error && <p role="alert">{error}</p>}
        <button
          className="primary"
          type="submit"
          disabled={pending || !token.trim() || wait > 0}
        >
          {pending ? "로그인 중" : wait ? `${wait}초 후 재시도` : "로그인"}
          <ArrowRight size={16} aria-hidden="true" />
        </button>
      </form>
    </main>
  );
}
