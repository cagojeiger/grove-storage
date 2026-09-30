import {
  DialogContent,
  DialogActions,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
  IconButton,
} from "@mui/material";

import { FormEvent, useState } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { KeyRound, Trash2 } from "lucide-react";
import { ApiError, Session, identity, message, request } from "../../api/http";
import {
  Credential,
  Issued,
  field,
  identityPage,
  isCredential,
  isIssued,
  isChanged,
  identityRequest,
} from "../../api/identity";
import { Dialog } from "../../design/Dialog";
import { time } from "../../design/format";
import { IssuedToken } from "../access/IssuedToken";

export function PersonalTokens({
  session,
}: {
  session: Session & { principal: "user" };
}) {
  const cache = useQueryClient();
  const [target, setTarget] = useState<"issue" | Credential | null>(null);
  const query = useInfiniteQuery({
    queryKey: ["me", "tokens", session.user_id],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) =>
      identityPage(
        "/me/tokens",
        (value): value is Credential =>
          isCredential(value) && value.account_id === session.user_id,
        pageParam,
        signal,
      ),
    getNextPageParam: (page) => page.next_before ?? undefined,
    gcTime: 0,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <section className="storage-section" aria-label="My API tokens">
      <div className="section-heading">
        <h2>My API tokens</h2>
        <Button
          type="submit"
          startIcon={<KeyRound size={16} />}
          disabled={query.isError}
          onClick={() => setTarget("issue")}
        >
          Issue token
        </Button>
      </div>
      {query.isPending ? (
        <p role="status">Loading tokens...</p>
      ) : query.isError ? (
        <p role="alert">
          {message(query.error)}{" "}
          <Button type="submit" onClick={() => void query.refetch()}>
            Retry
          </Button>
        </p>
      ) : (
        <>
          {rows.map((token) => (
            <div className="token-row" key={token.id}>
              <div>
                <strong>{token.label}</strong>
                <p className="muted">{token.token_prefix}</p>
                <p className="muted">{token.id}</p>
              </div>
              <div>
                <span>
                  {token.revoked_at
                    ? "Revoked"
                    : Date.parse(token.expires_at) <= Date.now()
                      ? "Expired"
                      : "Active"}
                </span>
                <p className="muted">Expires {time(token.expires_at)}</p>
              </div>
              <IconButton
                type="submit"
                className="icon-button"
                title={`Revoke ${token.label}`}
                aria-label={`Revoke ${token.label}`}
                disabled={Boolean(token.revoked_at)}
                onClick={() => setTarget(token)}
              >
                <Trash2 size={16} />
              </IconButton>
            </div>
          ))}
          {!rows.length && <p className="empty">No API tokens.</p>}
          {query.hasNextPage && (
            <Button
              type="submit"
              disabled={query.isFetchingNextPage}
              onClick={() => void query.fetchNextPage()}
            >
              Load more
            </Button>
          )}
        </>
      )}
      {target && (
        <PersonalTokenDialog
          account={session.user_id}
          target={target}
          onClose={() => setTarget(null)}
          onSaved={async () => {
            await cache.invalidateQueries({
              queryKey: ["me", "tokens", session.user_id],
            });
          }}
        />
      )}
    </section>
  );
}

function PersonalTokenDialog({
  account,
  target,
  onClose,
  onSaved,
}: {
  account: string;
  target: "issue" | Credential;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [unknown, setUnknown] = useState(false);
  const [error, setError] = useState("");
  const [issued, setIssued] = useState<Issued | null>(null);
  const [confirmed, setConfirmed] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || unknown) return;
    const data = new FormData(event.currentTarget);
    event.currentTarget.reset();
    setBusy(true);
    setError("");
    try {
      if (target === "issue") {
        const value = await request<unknown>(`${identity}/me/tokens`, {
          method: "POST",
          body: JSON.stringify({
            label: field(data, "label").trim(),
            expires_in_days: Number(data.get("days")),
            current_password: field(data, "current_password"),
          }),
        });
        if (!isIssued(value) || value.account_id !== account)
          throw new Error("Invalid token response");
        setIssued(value);
      } else {
        if (!confirmed) return;
        await identityRequest(
          `/me/tokens/${encodeURIComponent(target.id)}`,
          isChanged,
          { method: "DELETE" },
        );
        onClose();
      }
      await onSaved();
    } catch (failure) {
      if (
        failure instanceof ApiError &&
        [400, 401, 403, 404, 409, 429].includes(failure.status)
      ) {
        setError(
          failure.status === 401 && target === "issue"
            ? "Current password is incorrect or the session expired."
            : message(failure),
        );
      } else {
        setUnknown(true);
        setError(
          "The result is unknown. Close and review your tokens before trying again.",
        );
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      title={
        issued
          ? "Save token"
          : target === "issue"
            ? "Issue API token"
            : "Revoke API token"
      }
      busy={busy}
      closeDisabled={Boolean(issued)}
      onClose={onClose}
    >
      {issued ? (
        <DialogContent>
          <IssuedToken value={issued} onDone={onClose} />
        </DialogContent>
      ) : (
        <form
          onSubmit={(event) => {
            void submit(event);
          }}
        >
          <DialogContent>
            <fieldset className="storage-form" disabled={busy || unknown}>
              {target === "issue" ? (
                <>
                  <TextField
                    name="label"
                    required
                    label={"Label"}
                    slotProps={{ htmlInput: { maxLength: 80 } }}
                  />
                  <TextField
                    name="days"
                    type="number"
                    defaultValue={90}
                    required
                    label={"Expires in days"}
                    slotProps={{ htmlInput: { min: 1, max: 90 } }}
                  />
                  <TextField
                    name="current_password"
                    type="password"
                    autoComplete="current-password"
                    required
                    label={"Current password"}
                    className="full-field"
                  />
                </>
              ) : (
                <FormControlLabel
                  className="check-field full-field"
                  control={
                    <Checkbox
                      checked={confirmed}
                      onChange={(event) => setConfirmed(event.target.checked)}
                    />
                  }
                  label={<>Revoke {target.label}</>}
                />
              )}
            </fieldset>
            {error && (
              <p role="alert" className="form-error">
                {error}
              </p>
            )}
          </DialogContent>
          <DialogActions>
            <Button type="button" disabled={busy} onClick={onClose}>
              {unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button
              type="submit"
              variant="contained"
              color={target === "issue" ? "primary" : "error"}
              className="primary"
              disabled={busy || unknown || (target !== "issue" && !confirmed)}
            >
              {busy ? "Saving..." : target === "issue" ? "Issue" : "Revoke"}
            </Button>
          </DialogActions>
        </form>
      )}
    </Dialog>
  );
}
