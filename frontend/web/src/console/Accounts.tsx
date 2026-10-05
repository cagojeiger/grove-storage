import { useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Box,
  Button,
  Chip,
  Divider,
  Link,
  Stack,
  Typography,
} from "@mui/material";
import { DataGrid, type GridColDef } from "@mui/x-data-grid";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type Account,
  field,
  identityRequest,
  isAccount,
} from "../api/identity";
import { type Session } from "../api/http";
import { cspNonce } from "../app/csp";
import { isAccountPage, useAccountList } from "../features/access/accountList";
import { activityLink } from "../features/activity/filters";
import { clearSession } from "../auth/session";
import {
  AccountEdit,
  AccountSetup,
  type AccountAction,
} from "./AccountActions";
import { Page, Properties, QueryState, Refresh } from "./ui";
import { Tokens } from "./Tokens";

export function Accounts({ id, session }: { id?: string; session: Session }) {
  const cache = useQueryClient();
  const listing = useAccountList();
  const [create, setCreate] = useState(false);
  const [setup, setSetup] = useState(false);
  const [action, setAction] = useState<AccountAction | null>(null);
  const query = useQuery({
    queryKey: ["access", "account", id],
    enabled: Boolean(id),
    queryFn: ({ signal }) =>
      identityRequest(
        `/accounts/${encodeURIComponent(id!)}`,
        (value): value is Account => isAccount(value) && value.id === id,
        { signal },
      ),
  });
  const list = useQuery({
    queryKey: ["access", "accounts", listing.query],
    enabled: !id,
    queryFn: ({ signal }) =>
      identityRequest(`/accounts?${listing.query}`, isAccountPage, { signal }),
  });
  const account = !query.isError ? query.data : undefined;
  async function refresh() {
    await cache.invalidateQueries({ queryKey: ["access"] });
    await cache.invalidateQueries({ queryKey: ["me"] });
    await cache.invalidateQueries({ queryKey: ["session"] });
  }
  const columns: GridColDef<Account>[] = [
    {
      field: "display_name",
      headerName: "Account",
      flex: 1,
      minWidth: 180,
      renderCell: ({ row }) => (
        <Link href={listing.href(`#accounts/${row.id}`)}>
          {row.display_name}
        </Link>
      ),
    },
    { field: "username", headerName: "Username", flex: 1, minWidth: 140 },
    {
      field: "role",
      headerName: "Role",
      width: 110,
      renderCell: ({ row }) => (
        <Chip label={row.role} size="small" variant="outlined" />
      ),
    },
    {
      field: "status",
      headerName: "Status",
      width: 150,
      valueGetter: (_, row) =>
        row.deleted_at
          ? "Deleted"
          : !row.is_active
            ? "Disabled"
            : row.password_ready
              ? "Active"
              : "Pending setup",
    },
  ];
  return (
    <Page
      fill={!id}
      title={id ? (account?.display_name ?? "Account") : "Accounts"}
      parent={
        id ? { label: "Accounts", href: listing.href("#accounts") } : undefined
      }
      actions={
        <>
          <Refresh
            label="Refresh accounts"
            onClick={() => void refresh()}
            disabled={id ? query.isFetching : list.isFetching}
          />
          {!id && (
            <Button variant="contained" onClick={() => setCreate(true)}>
              Create account
            </Button>
          )}
        </>
      }
    >
      {!id ? (
        <>
          <Stack
            direction={{ xs: "column", sm: "row" }}
            spacing={2}
            component="form"
            onSubmit={(event) => {
              event.preventDefault();
              const data = new FormData(event.currentTarget);
              listing.update({ q: field(data, "q") });
            }}
          >
            <TextField
              key={listing.search}
              type="search"
              fullWidth
              size="small"
              label="Search accounts"
              name="q"
              defaultValue={listing.search}
            />
            <TextField
              size="small"
              select
              slotProps={{ select: { native: true } }}
              label="Account role"
              value={listing.role}
              onChange={(event) =>
                listing.update({
                  role:
                    event.target.value === "all" ? null : event.target.value,
                })
              }
              sx={{ minWidth: 120 }}
            >
              {["all", "reader", "writer", "admin"].map((role) => (
                <option key={role} value={role}>
                  {role === "all" ? "All roles" : role}
                </option>
              ))}
            </TextField>
            <TextField
              size="small"
              select
              slotProps={{ select: { native: true } }}
              label="Account status"
              value={listing.status}
              onChange={(event) =>
                listing.update({ status: event.target.value })
              }
              sx={{ minWidth: 130 }}
            >
              {["current", "active", "disabled", "deleted", "all"].map(
                (status) => (
                  <option key={status} value={status}>
                    {status}
                  </option>
                ),
              )}
            </TextField>
            <Button type="submit">Search</Button>
          </Stack>
          <QueryState
            pending={list.isPending}
            error={list.error}
            retry={() => void list.refetch()}
          />
          {!list.isError && (
            <>
              <Box
                sx={{
                  display: "flex",
                  flexDirection: "column",
                  minHeight: 200,
                  height: 0,
                  flex: "1 1 0",
                  minWidth: 0,
                }}
              >
                <DataGrid
                  nonce={cspNonce}
                  aria-label="Accounts"
                  rows={list.data?.items ?? []}
                  columns={columns}
                  loading={list.isPending}
                  disableColumnFilter
                  disableColumnSorting
                  disableRowSelectionOnClick
                  hideFooter
                  localeText={{
                    noRowsLabel:
                      listing.search || listing.role !== "all"
                        ? "No matching accounts."
                        : "No accounts found.",
                  }}
                />
              </Box>
              <Stack
                component="nav"
                aria-label="Account pagination"
                direction="row"
                spacing={{ xs: 1, sm: 2 }}
                sx={{ justifyContent: "flex-end", alignItems: "center" }}
              >
                <TextField
                  size="small"
                  select
                  slotProps={{ select: { native: true } }}
                  label="Page size"
                  sx={{ minWidth: 112 }}
                  value={listing.limit}
                  onChange={(event) =>
                    listing.update({ limit: event.target.value })
                  }
                >
                  {[20, 50, 100].map((limit) => (
                    <option key={limit} value={String(limit)}>
                      {limit}
                    </option>
                  ))}
                </TextField>
                <Button
                  aria-label="Previous page"
                  disabled={!list.data?.previous_after || list.isFetching}
                  onClick={() =>
                    listing.update({ after: list.data?.previous_after ?? null })
                  }
                >
                  Previous
                </Button>
                <Button
                  aria-label="Next page"
                  disabled={!list.data?.next_before || list.isFetching}
                  onClick={() =>
                    listing.update({ before: list.data?.next_before ?? null })
                  }
                >
                  Next
                </Button>
              </Stack>
            </>
          )}
        </>
      ) : (
        <>
          <QueryState
            pending={query.isPending}
            error={query.error}
            retry={() => void query.refetch()}
          />
          {account && (
            <>
              <Properties
                values={{
                  "Account ID": account.id,
                  Username: account.username,
                  Role: account.role,
                  Status: account.deleted_at
                    ? "Deleted"
                    : account.is_active
                      ? "Active"
                      : "Disabled",
                  Password: account.password_ready
                    ? "Configured"
                    : "Pending setup",
                }}
              />
              <Stack
                direction="row"
                spacing={1}
                useFlexGap
                sx={{ flexWrap: "wrap" }}
              >
                <Button
                  disabled={Boolean(account.deleted_at)}
                  onClick={() => setAction("name")}
                >
                  Edit name
                </Button>
                <Button
                  disabled={Boolean(account.deleted_at)}
                  onClick={() => setAction("role")}
                >
                  Change role
                </Button>
                <Button href={activityLink(account.id)}>
                  View account actions
                </Button>
              </Stack>
              <Divider />
              {!account.password_ready && (
                <Stack spacing={2}>
                  <Typography component="h2" variant="h6">
                    Password setup
                  </Typography>
                  <Button
                    sx={{ alignSelf: "flex-start" }}
                    disabled={!account.is_active || Boolean(account.deleted_at)}
                    onClick={() => setSetup(true)}
                  >
                    Issue setup link
                  </Button>
                  <Divider />
                </Stack>
              )}
              <Tokens
                accountId={account.id}
                disabled={!account.is_active || Boolean(account.deleted_at)}
              />
              <Divider />
              <Stack spacing={2}>
                <Typography component="h2" variant="h6">
                  Account status
                </Typography>
                <Stack direction="row" spacing={2}>
                  <Button
                    color={account.is_active ? "warning" : "primary"}
                    variant="outlined"
                    disabled={Boolean(account.deleted_at)}
                    onClick={() => setAction("active")}
                  >
                    {account.is_active ? "Disable account" : "Enable account"}
                  </Button>
                  <Button
                    color="error"
                    disabled={Boolean(account.deleted_at)}
                    onClick={() => setAction("delete")}
                  >
                    Delete account
                  </Button>
                </Stack>
              </Stack>
            </>
          )}
        </>
      )}
      {action && account && (
        <AccountEdit
          account={account}
          mode={action}
          self={account.id === session.user_id}
          onClose={() => setAction(null)}
          onSaved={async () => {
            if (
              account.id === session.user_id &&
              (action === "delete" ||
                (action === "active" && account.is_active))
            )
              clearSession(cache);
            else await refresh();
          }}
        />
      )}
      {(create || setup) && (
        <AccountSetup
          account={setup ? account : undefined}
          onClose={() => {
            setCreate(false);
            setSetup(false);
            void refresh();
          }}
          onSaved={async (target) => {
            await refresh();
            location.hash = listing.href(`#accounts/${target}`);
          }}
        />
      )}
    </Page>
  );
}
