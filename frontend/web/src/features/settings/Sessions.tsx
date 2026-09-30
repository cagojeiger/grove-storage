import { IconButton, Button } from "@mui/material";
import { Input } from "../../design/Fields";
import { useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut, Pencil, RefreshCw } from "lucide-react";
import { ApiError, Session, message } from "../../api/http";
import {
  Account,
  identityPage,
  identityRequest,
  isAccount,
  isChanged,
  isObject,
} from "../../api/identity";
import { clearSession } from "../../auth/session";
import { Dialog } from "../../design/Dialog";
import { useAction } from "../access/useAction";
import { time } from "../../design/format";
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
  const [editing, setEditing] = useState(false);
  const passwordSession = session.credential_id === null;
  const profile = useQuery({
    queryKey: ["me", "profile", session.user_id],
    enabled: passwordSession,
    queryFn: ({ signal }) => identityRequest("/me", isAccount, { signal }),
    gcTime: 0,
  });
  const query = useInfiniteQuery({
    queryKey: ["sessions", session.session_id],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) =>
      identityPage("/me/sessions", isSession, pageParam, signal),
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
        <IconButton type="submit"
          className="icon-button"
          title="Refresh sessions"
          aria-label="Refresh sessions"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          <RefreshCw size={18} />
        </IconButton>
      </div>
      {passwordSession && profile.isPending && <p role="status">Loading profile...</p>}
      {passwordSession && profile.isError && <p role="alert">{message(profile.error)} <Button type="submit" onClick={() => void profile.refetch()}>Retry</Button></p>}
      {profile.data && <div className="section-heading">
        <h2>{profile.data.display_name}</h2>
        <IconButton type="submit" className="icon-button" title="Edit my name" aria-label="Edit my name" onClick={() => setEditing(true)}><Pencil size={16} /></IconButton>
      </div>}
      <dl className="detail-fields">
        {profile.data && <div><dt>Username</dt><dd>{profile.data.username}</dd></div>}
        <div>
          <dt>Account</dt>
          <dd>{session.user_id}</dd>
        </div>
        <div>
          <dt>Role</dt>
          <dd>{session.role}</dd>
        </div>
      </dl>
      {editing && profile.data && <EditProfile account={profile.data} onClose={() => setEditing(false)}
        onSaved={async () => { await cache.invalidateQueries({ queryKey: ["me", "profile"] }); }} />}
      {passwordSession && <section className="storage-section" aria-label="Security">
        <div className="section-heading"><h2>Security</h2><a href="#settings/security">Change password</a></div>
      </section>}
      {passwordSession && <PersonalTokens session={session} />}
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
                        : "Password sign-in"}
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
                  <IconButton type="submit"
                    className="icon-button danger"
                    color="error"
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
                  </IconButton>
                </div>
              );
            })}
            {!rows.length && <p className="empty">No sessions found.</p>}
            {query.hasNextPage && (
              <Button type="submit"
                disabled={query.isFetching}
                onClick={() => void query.fetchNextPage()}
              >
                {query.isFetching ? "Loading..." : "Load more"}
              </Button>
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

function EditProfile({ account, onClose, onSaved }: {
  account: Account;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const action = useAction();
  const [name, setName] = useState(account.display_name);
  return <Dialog title="Edit my name" busy={action.busy} onClose={onClose}>
    <form onSubmit={(event) => {
      event.preventDefault();
      void action.run(async () => {
        await identityRequest("/me", isChanged, {
          method: "PATCH", body: JSON.stringify({ display_name: name.trim() }),
        });
        onClose();
        await onSaved();
      });
    }}>
      <fieldset className="storage-form" disabled={action.busy || action.unknown}>
        <label className="full-field">Name<Input value={name} onChange={event => setName(event.target.value)} required maxLength={80} /></label>
      </fieldset>
      {action.error && <p role="alert" className="form-error">{action.error}</p>}
      <div className="dialog-actions">
        <Button type="button" disabled={action.busy} onClick={onClose}>{action.unknown ? "Close and review" : "Cancel"}</Button>
        <Button type="submit" variant="contained" className="primary" disabled={action.busy || action.unknown || !name.trim() || name.trim() === account.display_name}>Save</Button>
      </div>
    </form>
  </Dialog>;
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
        <Button type="submit" disabled={action.busy} onClick={onClose}>
          {action.unknown ? "Close and review" : "Cancel"}
        </Button>
        <Button type="submit" color="error"
          className="danger"
          disabled={action.busy || action.unknown}
          onClick={() =>
            void action.run(async () => {
              await identityRequest(
                `/me/sessions/${encodeURIComponent(row.id)}`,
                isChanged,
                { method: "DELETE" },
              );
              await onRevoked();
            })
          }
        >
          {action.busy ? "Revoking..." : "Confirm revoke"}
        </Button>
      </div>
    </Dialog>
  );
}
