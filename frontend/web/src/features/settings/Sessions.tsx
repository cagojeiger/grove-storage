import { useState } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut, RefreshCw } from "lucide-react";
import { ApiError, Session, message } from "../../api/http";
import {
  identityPage,
  identityRequest,
  isChanged,
  isObject,
} from "../../api/identity";
import { clearSession } from "../../auth/session";
import { Dialog } from "../../design/Dialog";
import { useAction } from "../access/useAction";
import { time } from "../../design/format";
import { PasswordChange } from "./PasswordChange";
import { PersonalTokens } from "./PersonalTokens";

type LoginSession = {
  id: string;
  credential_id: string | null;
  created_at: string;
  expires_at: string;
  revoked_at: string | null;
};
function isSession(v: unknown): v is LoginSession {
  return (
    isObject(v) &&
    typeof v.id === "string" &&
    (v.credential_id === null || typeof v.credential_id === "string") &&
    ["created_at", "expires_at"].every(
      (k) => typeof v[k] === "string" && Number.isFinite(Date.parse(v[k])),
    ) &&
    (v.revoked_at === null || typeof v.revoked_at === "string")
  );
}
export function Sessions({ session }: { session: Session }) {
  const cache = useQueryClient();
  const [selected, setSelected] = useState<LoginSession | null>(null);
  const query = useInfiniteQuery({
    queryKey: ["sessions", session.session_id],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) =>
      identityPage("/sessions", isSession, pageParam, signal),
    getNextPageParam: (page) => page.next_before,
    gcTime: 0,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <main className="overview settings">
      <div className="page-heading">
        <div>
          <p className="eyebrow">MY ACCOUNT</p>
          <h1>My account</h1>
        </div>
        <button
          className="icon-button"
          title="Refresh sessions"
          aria-label="Refresh sessions"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          <RefreshCw size={18} />
        </button>
      </div>
      <dl className="detail-fields">
        <div>
          <dt>Account</dt>
          <dd>{session.principal === "root" ? "Root" : session.user_id}</dd>
        </div>
        <div>
          <dt>Role</dt>
          <dd>{session.role}</dd>
        </div>
      </dl>
      {session.principal === "user" && session.credential_id === null && <PasswordChange />}
      {session.principal === "user" && session.credential_id === null && <PersonalTokens session={session} />}
      <section className="storage-section" aria-label="My sessions">
        <h2>My sessions</h2>
        {query.isPending ? (
          <p role="status">Loading sessions...</p>
        ) : query.isError ? (
          <p role="alert">{message(query.error)}</p>
        ) : (
          <>
            {rows.map((row) => {
              const current = row.id === session.session_id;
              const state = row.revoked_at
                ? "Revoked"
                : Date.parse(row.expires_at) <= Date.now()
                  ? "Expired"
                  : "Active";
              return (
                <div className="session-row" key={row.id}>
                  <div>
                    <strong>
                      {current ? "Current session" : "Browser session"}
                    </strong>
                    <code>{row.id}</code>
                    <span className="muted">
                      {row.credential_id
                        ? `Token: ${row.credential_id}`
                        : session.principal === "root" ? "Root sign-in" : "Password sign-in"}
                    </span>
                  </div>
                  <div>
                    <span>{state}</span>
                    <span className="muted">
                      Created {time(row.created_at)}
                    </span>
                    <span className="muted">
                      Expires {time(row.expires_at)}
                    </span>
                  </div>
                  <button
                    className="icon-button danger"
                    title={
                      current
                        ? "Revoke current session"
                        : `Revoke session ${row.id}`
                    }
                    aria-label={
                      current
                        ? "Revoke current session"
                        : `Revoke session ${row.id}`
                    }
                    disabled={state !== "Active"}
                    onClick={() => setSelected(row)}
                  >
                    <LogOut size={17} />
                  </button>
                </div>
              );
            })}
            {!rows.length && <p className="empty">No sessions found.</p>}
            {query.hasNextPage && (
              <button
                disabled={query.isFetching}
                onClick={() => void query.fetchNextPage()}
              >
                {query.isFetching ? "Loading..." : "Load more"}
              </button>
            )}
          </>
        )}
      </section>
      {selected && (
        <RevokeSession
          row={selected}
          current={selected.id === session.session_id}
          onClose={() => setSelected(null)}
          onRevoked={async () => {
            if (selected.id === session.session_id) clearSession(cache);
            else {
              setSelected(null);
              await cache.invalidateQueries({ queryKey: ["sessions"] });
            }
          }}
        />
      )}
    </main>
  );
}
function RevokeSession({
  row,
  current,
  onClose,
  onRevoked,
}: {
  row: LoginSession;
  current: boolean;
  onClose: () => void;
  onRevoked: () => Promise<void>;
}) {
  const action = useAction((error) =>
    error instanceof ApiError && error.outcome === "not_applied"
      ? message(error)
      : "The outcome is unknown. Close and refresh the session list before trying again.",
  );
  return (
    <Dialog title="Revoke session" busy={action.busy} onClose={onClose}>
      <p>
        {current
          ? "This signs you out of the current console session."
          : "This ends the selected browser session. The account token remains valid."}
      </p>
      <code className="key-value">{row.id}</code>
      {action.error && <p role="alert">{action.error}</p>}
      <div className="dialog-actions">
        <button disabled={action.busy} onClick={onClose}>
          {action.unknown ? "Close and review" : "Cancel"}
        </button>
        <button
          className="danger"
          disabled={action.busy || action.unknown}
          onClick={() =>
            void action.run(async () => {
              await identityRequest(
                `/sessions/${encodeURIComponent(row.id)}`,
                isChanged,
                { method: "DELETE" },
              );
              await onRevoked();
            })
          }
        >
          {action.busy ? "Revoking..." : "Confirm revoke"}
        </button>
      </div>
    </Dialog>
  );
}
