import { useEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowRight } from "lucide-react";
import { ApiError, identity, message, request } from "../api/http";
import {
  Issued,
  MasterSession,
  identityMessage,
  field,
  identityRequest,
  isMaster,
  isObject,
  isIssued,
} from "../api/identity";
import { IssuedToken } from "../features/access/IssuedToken";

export function MasterSetup() {
  const [session, setSession] = useState<MasterSession | null>(null);
  const [issued, setIssued] = useState<Issued | null>(null);
  const [checking, setChecking] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [unknown, setUnknown] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [wait, setWait] = useState(0);
  const lock = useRef(false);
  useEffect(() => {
    const abort = new AbortController();
    void identityRequest("/master/session", isMaster, { signal: abort.signal })
      .then((value) => {
        if (!abort.signal.aborted) setSession(value);
      })
      .catch((e: unknown) => {
        if (
          !abort.signal.aborted &&
          !(e instanceof ApiError && [401, 404].includes(e.status))
        )
          setError(message(e));
      })
      .finally(() => {
        if (!abort.signal.aborted) setChecking(false);
      });
    return () => abort.abort();
  }, []);
  useEffect(() => {
    if (!wait) return;
    const timer = setTimeout(() => setWait(wait - 1), 1000);
    return () => clearTimeout(timer);
  }, [wait]);
  async function leave() {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    try {
      await request(`${identity}/master/session`, { method: "DELETE" });
      location.hash = "";
    } catch (e) {
      if (e instanceof ApiError && [401, 404].includes(e.status))
        location.hash = "";
      else setError(identityMessage(e));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  return (
    <main className="login master-setup">
      <h1>Grove Storage</h1>
      <h2>
        {issued
          ? "Admin access ready"
          : session?.initialized
            ? "Recover Admin access"
            : "Setup & recovery"}
      </h2>
      {checking ? (
        <p role="status">Checking setup session...</p>
      ) : issued ? (
        <IssuedToken
          value={issued}
          onDone={() => {
            setIssued(null);
            location.hash = "";
          }}
        />
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (lock.current || unknown || wait) return;
            const data = new FormData(e.currentTarget);
            const tokenInput = e.currentTarget.elements.namedItem("token");
            if (tokenInput instanceof HTMLInputElement) tokenInput.value = "";
            lock.current = true;
            setBusy(true);
            setError("");
            void (async () => {
              try {
                if (!session) {
                  await identityRequest(
                    "/master/session",
                    (v): v is { principal: "master" } =>
                      isObject(v) && v.principal === "master",
                    {
                      method: "POST",
                      body: JSON.stringify({
                        token: field(data, "token").trim(),
                      }),
                    },
                  );
                  setSession(
                    await identityRequest("/master/session", isMaster),
                  );
                } else {
                  if (session.initialized && !confirmed) return;
                  const result = await identityRequest(
                    session.initialized
                      ? "/master/recover"
                      : "/master/bootstrap",
                    (v): v is Issued =>
                      isIssued(v) &&
                      typeof v.user_id === "string" &&
                      (!session.initialized ||
                        v.user_id === field(data, "user_id")),
                    {
                      method: "POST",
                      body: JSON.stringify(
                        session.initialized
                          ? { user_id: data.get("user_id"), confirm: true }
                          : {
                              display_name: field(data, "display_name").trim(),
                            },
                      ),
                    },
                  );
                  setSession(null);
                  setIssued(result);
                }
              } catch (e) {
                setError(identityMessage(e));
                setUnknown(
                  !(e instanceof ApiError) || e.outcome !== "not_applied",
                );
                if (e instanceof ApiError && e.status === 401) setSession(null);
                if (e instanceof ApiError && e.status === 429)
                  setWait(e.retryAfter ?? 60);
              } finally {
                lock.current = false;
                setBusy(false);
              }
            })();
          }}
        >
          {!session ? (
            <label>
              Root token
              <input
                name="token"
                type="password"
                autoComplete="off"
                required
                disabled={busy || unknown}
              />
            </label>
          ) : session.initialized ? (
            <>
              <label>
                Admin user ID
                <input
                  name="user_id"
                  required
                  pattern="[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}"
                  disabled={busy || unknown}
                />
              </label>
              <label className="check-field">
                <input
                  type="checkbox"
                  checked={confirmed}
                  onChange={(e) => setConfirmed(e.target.checked)}
                  disabled={busy || unknown}
                />
                Replace this Admin's tokens and revoke its sessions
              </label>
            </>
          ) : (
            <label>
              Admin name
              <input
                name="display_name"
                required
                maxLength={80}
                disabled={busy || unknown}
              />
            </label>
          )}
          {error && <p role="alert">{error}</p>}
          <button
            className="primary"
            disabled={
              busy ||
              unknown ||
              wait > 0 ||
              Boolean(session?.initialized && !confirmed)
            }
          >
            {busy
              ? "Working..."
              : wait
                ? `Retry in ${wait}s`
                : !session
                  ? "Verify Root token"
                  : session.initialized
                    ? "Recover access"
                    : "Create Admin"}
            <ArrowRight size={16} />
          </button>
        </form>
      )}
      {!issued && (
        <button
          className="back-link"
          disabled={busy}
          onClick={() => void leave()}
        >
          <ArrowLeft size={16} />
          Back to sign in
        </button>
      )}
    </main>
  );
}
