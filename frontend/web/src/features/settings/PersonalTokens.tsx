import {
  DialogContent,
  DialogActions,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
  IconButton,
  Alert,
  List,
  ListItem,
  ListItemText,
  Stack,
  Tooltip,
  Typography,
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
    <Stack component="section" aria-label="My API tokens" spacing={2}>
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={2}
        sx={{ alignItems: { sm: "center" }, justifyContent: "space-between" }}
      >
        <Typography component="h2" variant="h6">
          My API tokens
        </Typography>
        <Button
          type="submit"
          startIcon={<KeyRound size={16} />}
          disabled={query.isError}
          onClick={() => setTarget("issue")}
        >
          Issue token
        </Button>
      </Stack>
      {query.isPending ? (
        <Typography role="status">Loading tokens...</Typography>
      ) : query.isError ? (
        <Alert severity="error">
          {message(query.error)}{" "}
          <Button type="submit" onClick={() => void query.refetch()}>
            Retry
          </Button>
        </Alert>
      ) : (
        <>
          <List disablePadding aria-label="My API tokens">
            {rows.map((token) => (
              <ListItem
                key={token.id}
                divider
                disableGutters
                secondaryAction={
                  <Tooltip title={`Revoke ${token.label}`}>
                    <span>
                      <IconButton
                        edge="end"
                        aria-label={`Revoke ${token.label}`}
                        disabled={Boolean(token.revoked_at)}
                        onClick={() => setTarget(token)}
                      >
                        <Trash2 size={16} />
                      </IconButton>
                    </span>
                  </Tooltip>
                }
              >
                <ListItemText
                  primary={token.label}
                  sx={{ overflowWrap: "anywhere" }}
                  slotProps={{ secondary: { component: "div" } }}
                  secondary={
                    <Stack spacing={0.5}>
                      <Typography variant="body2">
                        {token.token_prefix}
                      </Typography>
                      <Typography variant="body2">{token.id}</Typography>
                      <Typography variant="body2">
                        {token.revoked_at
                          ? "Revoked"
                          : Date.parse(token.expires_at) <= Date.now()
                            ? "Expired"
                            : "Active"}
                      </Typography>
                      <Typography variant="body2">
                        Expires {time(token.expires_at)}
                      </Typography>
                    </Stack>
                  }
                />
              </ListItem>
            ))}
          </List>
          {!rows.length && (
            <Typography color="text.secondary">No API tokens.</Typography>
          )}
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
    </Stack>
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
        <IssuedToken value={issued} onDone={onClose} />
      ) : (
        <form
          onSubmit={(event) => {
            void submit(event);
          }}
        >
          <DialogContent>
            <Stack spacing={3}>
              {target === "issue" ? (
                <>
                  <TextField
                    name="label"
                    disabled={busy || unknown}
                    required
                    label={"Label"}
                    slotProps={{ htmlInput: { maxLength: 80 } }}
                  />
                  <TextField
                    name="days"
                    disabled={busy || unknown}
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
                    disabled={busy || unknown}
                  />
                </>
              ) : (
                <FormControlLabel
                  disabled={busy || unknown}
                  control={
                    <Checkbox
                      checked={confirmed}
                      onChange={(event) => setConfirmed(event.target.checked)}
                    />
                  }
                  label={<>Revoke {target.label}</>}
                />
              )}
              {error && <Alert severity="error">{error}</Alert>}
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button type="button" disabled={busy} onClick={onClose}>
              {unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button
              type="submit"
              variant="contained"
              color={target === "issue" ? "primary" : "error"}
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
