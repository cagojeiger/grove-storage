import { useState } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { KeyRound, Trash2 } from "lucide-react";
import {
  Account,
  Credential,
  Issued,
  identityPage,
  field,
  identityRequest,
  isChanged,
  isCredential,
  isIssued,
} from "../../api/identity";
import { Dialog } from "../../design/Dialog";
import { message } from "../../api/http";
import { IssuedToken } from "./IssuedToken";
import { useAction } from "./useAction";
import { activityLink } from "../activity/filters";

export function Tokens({ account }: { account: Account }) {
  const cache = useQueryClient();
  const [dialog, setDialog] = useState<"issue" | Credential | null>(null);
  const query = useInfiniteQuery({
    queryKey: ["access", "tokens", account.id],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) =>
      identityPage(
        `/accounts/${encodeURIComponent(account.id)}/credentials`,
        (value): value is Credential =>
          isCredential(value) && value.account_id === account.id,
        pageParam,
        signal,
      ),
    getNextPageParam: (page) => page.next_before ?? undefined,
  });
  return (
    <section className="storage-section" aria-label="Management tokens">
      <div className="section-heading">
        <h2>Management tokens</h2>
        <button
          className="action-button"
          disabled={
            !account.is_active || Boolean(account.deleted_at) || query.isError
          }
          onClick={() => setDialog("issue")}
        >
          <KeyRound size={16} />
          Issue token
        </button>
      </div>
      {query.isPending ? (
        <p role="status">Loading tokens...</p>
      ) : query.isError ? (
        <p role="alert">
          {message(query.error)}{" "}
          <button onClick={() => void query.refetch()}>Retry</button>
        </p>
      ) : (
        <>
          {query.data.pages
            .flatMap((p) => p.items)
            .map((token) => (
              <div className="token-row" key={token.id}>
                <div>
                  <strong>{token.label}</strong>
                  <p className="muted">{token.token_prefix}</p>
                  <p className="muted">{token.id}</p>
                  <a href={activityLink(account.id, token.id)}>View token actions</a>
                </div>
                <div>
                  <span>
                    {token.revoked_at
                      ? "Revoked"
                      : Date.parse(token.expires_at) <= Date.now()
                        ? "Expired"
                        : "Active"}
                  </span>
                  <p className="muted">
                    Expires {new Date(token.expires_at).toLocaleString("en-US")}
                  </p>
                </div>
                <button
                  className="icon-button"
                  title={`Revoke ${token.label}`}
                  aria-label={`Revoke ${token.label}`}
                  disabled={Boolean(token.revoked_at)}
                  onClick={() => setDialog(token)}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            ))}
          {!query.data.pages.some((p) => p.items.length) && (
            <p className="empty">No tokens.</p>
          )}
          {query.hasNextPage && (
            <button
              disabled={query.isFetchingNextPage}
              onClick={() => void query.fetchNextPage()}
            >
              Load more tokens
            </button>
          )}
        </>
      )}
      {dialog && (
        <TokenDialog
          account={account}
          target={dialog}
          onClose={() => setDialog(null)}
          onSaved={async () => {
            await cache.invalidateQueries({
              queryKey: ["access", "tokens", account.id],
            });
            await cache.invalidateQueries({ queryKey: ["session"] });
          }}
        />
      )}
    </section>
  );
}

function TokenDialog({
  account,
  target,
  onClose,
  onSaved,
}: {
  account: Account;
  target: "issue" | Credential;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const state = useAction();
  const [issued, setIssued] = useState<Issued | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  return (
    <Dialog
      title={
        issued
          ? "Save token"
          : target === "issue"
            ? "Issue management token"
            : "Revoke token"
      }
      busy={state.busy}
      closeDisabled={Boolean(issued)}
      onClose={onClose}
    >
      {issued ? (
        <IssuedToken value={issued} onDone={onClose} />
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            const data = new FormData(e.currentTarget);
            void state.run(async () => {
              if (target === "issue") {
                const result = await identityRequest(
                  `/accounts/${encodeURIComponent(account.id)}/credentials`,
                  isIssued,
                  {
                    method: "POST",
                    body: JSON.stringify({
                      label: field(data, "label").trim(),
                      expires_in_days: Number(data.get("days")),
                    }),
                  },
                );
                if (result.account_id !== account.id)
                  throw new Error("Mismatched issued account");
                setIssued(result);
              } else {
                if (!confirmed) return;
                await identityRequest(
                  `/credentials/${encodeURIComponent(target.id)}`,
                  isChanged,
                  { method: "DELETE" },
                );
                onClose();
              }
              await onSaved();
            });
          }}
        >
          <fieldset
            className="storage-form"
            disabled={state.busy || state.unknown}
          >
            {target === "issue" ? (
              <>
                <label>
                  Label
                  <input name="label" required maxLength={80} />
                </label>
                <label>
                  Expires in days
                  <input
                    name="days"
                    type="number"
                    min={1}
                    max={90}
                    defaultValue={90}
                    required
                  />
                </label>
              </>
            ) : (
              <label className="check-field full-field">
                <input
                  type="checkbox"
                  checked={confirmed}
                  onChange={(e) => setConfirmed(e.target.checked)}
                />
                Revoke {target.label} and its sessions
              </label>
            )}
          </fieldset>
          {state.error && (
            <p role="alert" className="form-error">
              {state.error}
            </p>
          )}
          <div className="dialog-actions">
            <button type="button" disabled={state.busy} onClick={onClose}>
              {state.unknown ? "Close and review" : "Cancel"}
            </button>
            <button
              className="primary"
              disabled={
                state.busy ||
                state.unknown ||
                (target !== "issue" && !confirmed)
              }
            >
              {state.busy
                ? "Saving..."
                : target === "issue"
                  ? "Issue"
                  : "Revoke"}
            </button>
          </div>
        </form>
      )}
    </Dialog>
  );
}
