import { useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Button,
  Chip,
  IconButton,
  List,
  ListItem,
  ListItemText,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { History } from "lucide-react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import {
  type Credential,
  type Issued,
  field,
  identityPage,
  identityRequest,
  isChanged,
  isCredential,
  isIssued,
} from "../api/identity";
import { identity, request } from "../api/http";
import { useAction } from "../features/access/useAction";
import { time } from "../design/format";
import {
  Confirmation,
  FormDialog,
  Properties,
  QueryState,
  SecretDialog,
} from "./ui";
import { reauthenticationError } from "./identityModel";

export function Tokens({
  accountId,
  personal = false,
  disabled = false,
}: {
  accountId: string;
  personal?: boolean;
  disabled?: boolean;
}) {
  const cache = useQueryClient();
  const path = personal
    ? "/me/tokens"
    : `/accounts/${encodeURIComponent(accountId)}/credentials`;
  const [creating, setCreating] = useState(false);
  const [selected, setSelected] = useState<Credential | null>(null);
  const query = useInfiniteQuery({
    queryKey: ["tokens", accountId, personal],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) =>
      identityPage(
        path,
        (value): value is Credential =>
          isCredential(value) && value.account_id === accountId,
        pageParam,
        signal,
      ),
    getNextPageParam: (page) => page.next_before,
    gcTime: 0,
  });
  return (
    <Stack component="section" aria-label="Management API tokens" spacing={2}>
      <Stack
        direction="row"
        spacing={2}
        sx={{ justifyContent: "space-between", alignItems: "center" }}
      >
        <Typography component="h2" variant="h6">
          {personal ? "My API tokens" : "API tokens"}
        </Typography>
        <Button
          variant="outlined"
          disabled={disabled || query.isError}
          onClick={() => setCreating(true)}
        >
          Issue token
        </Button>
      </Stack>
      <QueryState
        pending={query.isPending}
        error={query.error}
        retry={() => void query.refetch()}
      />
      {!query.isError && (
        <List disablePadding aria-label="Management API tokens">
          {query.data?.pages
            .flatMap((page) => page.items)
            .map((row) => {
              const state = row.revoked_at
                ? "Revoked"
                : Date.parse(row.expires_at) <= Date.now()
                  ? "Expired"
                  : "Active";
              return (
                <ListItem
                  key={row.id}
                  divider
                  disableGutters
                  sx={{ gap: 2, flexWrap: { xs: "wrap", sm: "nowrap" } }}
                >
                  <ListItemText
                    primary={row.label}
                    secondary={`${row.token_prefix} · Expires ${time(row.expires_at)}`}
                    sx={{ overflowWrap: "anywhere" }}
                  />
                  <Chip label={state} size="small" variant="outlined" />
                  <Tooltip title="View token actions">
                    <IconButton
                      component="a"
                      aria-label="View token actions"
                      href={`#activity?account_id=${encodeURIComponent(accountId)}&credential_id=${encodeURIComponent(row.id)}`}
                    >
                      <History size={18} />
                    </IconButton>
                  </Tooltip>
                  <Button
                    color="error"
                    disabled={disabled || state !== "Active"}
                    onClick={() => setSelected(row)}
                  >
                    Revoke {row.label}
                  </Button>
                </ListItem>
              );
            })}
        </List>
      )}
      {!query.isPending &&
        !query.isError &&
        !query.data?.pages.some((page) => page.items.length) && (
          <Typography variant="body2" color="text.secondary">
            No tokens issued.
          </Typography>
        )}
      {query.hasNextPage && (
        <Button
          disabled={query.isFetching}
          onClick={() => void query.fetchNextPage()}
        >
          Load more
        </Button>
      )}
      {creating && (
        <TokenIssue
          accountId={accountId}
          path={path}
          personal={personal}
          onClose={() => {
            setCreating(false);
            void query.refetch();
          }}
          onSaved={() => void query.refetch()}
        />
      )}
      {selected && (
        <Confirmation
          title="Revoke token"
          text="This token and its browser sessions will stop working."
          confirm={selected.label}
          onClose={() => setSelected(null)}
          execute={async () => {
            await identityRequest(
              personal
                ? `/me/tokens/${encodeURIComponent(selected.id)}`
                : `/credentials/${encodeURIComponent(selected.id)}`,
              isChanged,
              { method: "DELETE" },
            );
            setSelected(null);
            await query.refetch();
            await cache.invalidateQueries({ queryKey: ["session"] });
          }}
        />
      )}
    </Stack>
  );
}
function TokenIssue({
  accountId,
  path,
  personal,
  onClose,
  onSaved,
}: {
  accountId: string;
  path: string;
  personal: boolean;
  onClose: () => void;
  onSaved: () => void;
}) {
  const action = useAction(reauthenticationError, true);
  const [issued, setIssued] = useState<Issued | null>(null);
  if (issued)
    return (
      <SecretDialog
        title="API token created"
        values={{ "Issued token": issued.token }}
        details={
          <Properties
            values={{
              "Account ID": accountId,
              Expires: time(issued.expires_at),
            }}
          />
        }
        onClose={onClose}
      />
    );
  return (
    <FormDialog
      title={personal ? "Issue personal token" : "Issue management token"}
      action={action}
      onClose={onClose}
      button="Issue"
      submit={async (data, form) => {
        const body = {
          label: field(data, "label").trim(),
          expires_in_days: Number(data.get("days")),
          current_password: field(data, "current_password"),
        };
        form.reset();
        const value = personal
          ? await request<unknown>(identity + path, {
              method: "POST",
              body: JSON.stringify(body),
            })
          : await identityRequest(path, isIssued, {
              method: "POST",
              body: JSON.stringify(body),
            });
        if (
          !isIssued(value) ||
          (personal
            ? value.user_id !== accountId && value.account_id !== accountId
            : value.account_id !== accountId)
        )
          throw new Error("Mismatched token response");
        setIssued(value);
        onSaved();
      }}
    >
      <TextField
        fullWidth
        label="Label"
        name="label"
        required
        slotProps={{ htmlInput: { maxLength: 80 } }}
      />
      <TextField
        fullWidth
        label="Expires in days"
        name="days"
        type="number"
        required
        defaultValue={30}
        slotProps={{ htmlInput: { min: 1, max: 90 } }}
      />
      <TextField
        fullWidth
        label="Current password"
        name="current_password"
        type="password"
        autoComplete="current-password"
        required
      />
    </FormDialog>
  );
}
