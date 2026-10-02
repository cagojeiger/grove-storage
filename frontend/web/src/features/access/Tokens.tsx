import {
  DialogContent,
  DialogActions,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
  Alert,
  Stack,
  Typography,
  Dialog,
  DialogTitle,
  useMediaQuery,
  useTheme,
  Chip,
} from "@mui/material";

import { useState, useId } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { KeyRound } from "lucide-react";
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

import { message } from "../../api/http";
import { TokenList } from "./TokenList";
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
    <Stack component="section" aria-label="Management API tokens" spacing={2}>
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={2}
        sx={{ alignItems: { sm: "center" }, justifyContent: "space-between" }}
      >
        <Typography component="h2" variant="h6">
          Management API tokens
        </Typography>
        <Button
          variant="contained"
          startIcon={<KeyRound size={16} />}
          disabled={
            !account.is_active || Boolean(account.deleted_at) || query.isError
          }
          onClick={() => setDialog("issue")}
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
          <TokenList
            tokens={query.data.pages.flatMap((page) => page.items)}
            onRevoke={setDialog}
            activityHref={(token) => activityLink(account.id, token.id)}
          />
          {!query.data.pages.some((p) => p.items.length) && (
            <Typography color="text.secondary">No tokens.</Typography>
          )}
          {query.hasNextPage && (
            <Button
              type="submit"
              disabled={query.isFetchingNextPage}
              onClick={() => void query.fetchNextPage()}
            >
              Load more tokens
            </Button>
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
    </Stack>
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
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));

  const state = useAction();
  const [issued, setIssued] = useState<Issued | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  return (
    <Dialog
      open
      fullWidth
      maxWidth={issued ? "sm" : "xs"}
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !state.busy && !issued) onClose();
      }}
    >
      <DialogTitle id={titleId}>
        {issued
          ? "API token created"
          : target === "issue"
            ? "Issue management token"
            : "Revoke token"}
      </DialogTitle>
      {issued ? (
        <IssuedToken
          value={issued}
          accountName={account.display_name}
          onDone={onClose}
        />
      ) : (
        <>
          <DialogContent dividers>
            <form
              id={`${titleId}-form`}
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
              <Stack spacing={3}>
                {target === "issue" ? (
                  <>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{
                        alignItems: "center",
                        flexWrap: "wrap",
                        overflowWrap: "anywhere",
                      }}
                    >
                      <Typography>{account.display_name}</Typography>
                      <Chip
                        size="small"
                        variant="outlined"
                        label={account.role}
                      />
                    </Stack>
                    <TextField
                      name="label"
                      disabled={state.busy || state.unknown}
                      required
                      label={"Label"}
                      slotProps={{ htmlInput: { maxLength: 80 } }}
                    />
                    <TextField
                      name="days"
                      disabled={state.busy || state.unknown}
                      type="number"
                      defaultValue={90}
                      required
                      label={"Expires in days"}
                      slotProps={{ htmlInput: { min: 1, max: 90 } }}
                    />
                  </>
                ) : (
                  <FormControlLabel
                    disabled={state.busy || state.unknown}
                    control={
                      <Checkbox
                        checked={confirmed}
                        onChange={(e) => setConfirmed(e.target.checked)}
                      />
                    }
                    label={<>Revoke {target.label} and its sessions</>}
                  />
                )}
                {state.error && <Alert severity="error">{state.error}</Alert>}
              </Stack>
            </form>
          </DialogContent>
          <DialogActions>
            <Button type="button" disabled={state.busy} onClick={onClose}>
              {state.unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button
              type="submit"
              form={`${titleId}-form`}
              variant="contained"
              color={target === "issue" ? "primary" : "error"}
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
            </Button>
          </DialogActions>
        </>
      )}
    </Dialog>
  );
}
