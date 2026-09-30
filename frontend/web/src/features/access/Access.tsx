import {
  Alert,
  Divider,
  Grid,
  IconButton,
  Button,
  Tooltip,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { useIsFetching, useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw, Pencil, History, Shield, Trash2 } from "lucide-react";
import { Account, identityRequest, isAccount } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { AccountAction, AccountDialog } from "./AccountDialog";
import { Tokens } from "./Tokens";
import { PasswordSetupIssue } from "./PasswordSetupIssue";
import { CreateUserDialog } from "./CreateUserDialog";
import { AccountsList } from "./AccountsList";
import { useAccountList } from "./accountList";
import { activityLink } from "../activity/filters";
import { Page } from "../../app/Page";

export function Access({
  route,
  currentUserId,
}: {
  route: string;
  currentUserId?: string;
}) {
  const cache = useQueryClient();
  const listing = useAccountList();
  const selected = route.startsWith("accounts/")
    ? route.slice("accounts/".length)
    : null;
  const [action, setAction] = useState<AccountAction | null>(null);
  const [creating, setCreating] = useState(false);
  const detail = useQuery({
    queryKey: ["access", "account", selected],
    enabled: Boolean(selected),
    queryFn: async ({ signal }) => {
      try {
        return await identityRequest(
          `/accounts/${encodeURIComponent(selected ?? "")}`,
          (v): v is Account => isAccount(v) && v.id === selected,
          { signal },
        );
      } catch (error) {
        if (error instanceof ApiError && error.status === 403)
          void cache.invalidateQueries({ queryKey: ["session"] });
        throw error;
      }
    },
  });
  const account = detail.data;
  const current = detail;
  const refreshing = useIsFetching({ queryKey: ["access"] }) > 0;
  async function refresh(createdId?: string) {
    if (createdId)
      window.location.hash = listing.href(
        `#accounts/${encodeURIComponent(createdId)}`,
      );
    await cache.invalidateQueries({ queryKey: ["session"] });
    await cache.invalidateQueries({ queryKey: ["access"] });
  }
  return (
    <Page
      title={
        selected
          ? current.isError
            ? "Account"
            : (account?.display_name ?? "Account")
          : "Accounts"
      }
      back={
        selected
          ? { label: "Accounts", href: listing.href("#accounts") }
          : undefined
      }
      actions={
        <>
          <Tooltip title="Refresh accounts">
            <span>
              <IconButton
                aria-label="Refresh accounts"
                disabled={refreshing}
                onClick={() => void refresh()}
              >
                <RefreshCw size={18} />
              </IconButton>
            </span>
          </Tooltip>
          {selected && account && !current.isError && (
            <>
              <Tooltip title="View account actions">
                <IconButton
                  href={activityLink(account.id)}
                  aria-label="View account actions"
                >
                  <History size={16} />
                </IconButton>
              </Tooltip>
              <Tooltip title="Edit name">
                <span>
                  <IconButton
                    aria-label="Edit name"
                    disabled={Boolean(account.deleted_at)}
                    onClick={() => setAction("name")}
                  >
                    <Pencil size={16} />
                  </IconButton>
                </span>
              </Tooltip>
              <Button
                startIcon={<Shield size={16} />}
                disabled={Boolean(account.deleted_at)}
                onClick={() => setAction("role")}
              >
                Change role
              </Button>
              <Button
                color={account.is_active ? "error" : "primary"}
                disabled={Boolean(account.deleted_at)}
                onClick={() => setAction("active")}
              >
                {account.is_active ? "Disable" : "Enable"}
              </Button>
              <Tooltip title="Delete account">
                <span>
                  <IconButton
                    color="error"
                    aria-label="Delete account"
                    disabled={Boolean(account.deleted_at)}
                    onClick={() => setAction("delete")}
                  >
                    <Trash2 size={16} />
                  </IconButton>
                </span>
              </Tooltip>
            </>
          )}
        </>
      }
    >
      {!selected ? (
        <AccountsList onCreate={() => setCreating(true)} />
      ) : current.isPending ? (
        <Typography role="status">Loading accounts...</Typography>
      ) : current.isError ? (
        <Alert severity="error">
          {current.error instanceof ApiError && current.error.status === 404
            ? "Account unavailable."
            : message(current.error)}{" "}
          <Button onClick={() => void current.refetch()}>Retry</Button>
          <Button href={listing.href("#accounts")}>Back to accounts</Button>
        </Alert>
      ) : selected ? (
        account ? (
          <>
            <Grid
              container
              component="dl"
              aria-label="Account details"
              spacing={3}
            >
              {[
                ["Account ID", account.id],
                ["Role", account.role],
                ["Username", account.username ?? "Not set"],
                [
                  "Status",
                  account.deleted_at
                    ? "Deleted"
                    : !account.is_active
                      ? "Disabled"
                      : account.password_ready
                        ? "Active"
                        : "Pending setup",
                ],
              ].map(([label, value]) => (
                <Grid key={label} size={{ xs: 12, sm: 6 }}>
                  <Typography
                    component="dt"
                    variant="body2"
                    color="text.secondary"
                  >
                    {label}
                  </Typography>
                  <Typography
                    component="dd"
                    sx={{ m: 0, overflowWrap: "anywhere" }}
                  >
                    {value}
                  </Typography>
                </Grid>
              ))}
            </Grid>
            <Divider />
            <PasswordSetupIssue key={account.id} account={account} />
            <Divider />
            <Tokens key={account.id} account={account} />
          </>
        ) : (
          <Typography>
            Account unavailable.{" "}
            <Button
              type="submit"
              onClick={() => {
                window.location.hash = listing.href("#accounts");
              }}
            >
              Back to accounts
            </Button>
          </Typography>
        )
      ) : null}
      {action && !current.isError && (
        <AccountDialog
          account={account}
          isSelf={Boolean(account && account.id === currentUserId)}
          action={action}
          onClose={() => setAction(null)}
          onSaved={refresh}
        />
      )}
      {creating && (
        <CreateUserDialog
          onClose={() => setCreating(false)}
          onCreated={refresh}
        />
      )}
    </Page>
  );
}
