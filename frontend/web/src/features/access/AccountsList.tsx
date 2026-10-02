import {
  TextField,
  Alert,
  Grid,
  Stack,
  Tooltip,
  Typography,
  IconButton,
  Button,
  Link,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Chip,
  Divider,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Plus, Search } from "lucide-react";
import { identityRequest } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { isAccountPage, useAccountList } from "./accountList";

export function AccountsList({ onCreate }: { onCreate: () => void }) {
  const compact = useMediaQuery(useTheme().breakpoints.down("sm"));
  const listing = useAccountList();
  const cache = useQueryClient();
  const query = useQuery({
    queryKey: ["access", "accounts", listing.query],
    queryFn: async ({ signal }) => {
      try {
        return await identityRequest(
          `/accounts?${listing.query}`,
          isAccountPage,
          { signal },
        );
      } catch (error) {
        if (error instanceof ApiError && error.status === 403)
          void cache.invalidateQueries({ queryKey: ["session"] });
        throw error;
      }
    },
  });
  const data = query.data;
  return (
    <Stack spacing={2}>
      <Grid container spacing={2} sx={{ alignItems: "center" }}>
        <Grid size={{ xs: 12, md: 5 }}>
          <Stack
            component="form"
            direction="row"
            spacing={1}
            onSubmit={(event) => {
              event.preventDefault();
              const value = new FormData(event.currentTarget).get("search");
              listing.update({
                q: typeof value === "string" ? value.trim() : "",
              });
            }}
          >
            <TextField
              size="small"
              label="Search accounts"
              key={listing.search}
              name="search"
              type="search"
              defaultValue={listing.search}
              slotProps={{
                htmlInput: { "aria-label": "Search accounts", maxLength: 80 },
              }}
            />
            <Tooltip title="Search">
              <IconButton type="submit" aria-label="Search">
                <Search size={18} />
              </IconButton>
            </Tooltip>
          </Stack>
        </Grid>
        <Grid size={{ xs: 6, md: 2 }}>
          <TextField
            size="small"
            value={listing.role}
            onChange={(e) =>
              listing.update({
                role: e.target.value === "all" ? null : e.target.value,
              })
            }
            label={"Role"}
            select
            slotProps={{
              htmlInput: { "aria-label": "Account role" },
              select: { native: true },
            }}
          >
            <option value="all">All roles</option>
            <option value="admin">Admin</option>
            <option value="writer">Writer</option>
            <option value="reader">Reader</option>
          </TextField>
        </Grid>
        <Grid size={{ xs: 6, md: 2 }}>
          <TextField
            size="small"
            value={listing.status}
            onChange={(e) => listing.update({ status: e.target.value })}
            label={"Status"}
            select
            slotProps={{
              htmlInput: { "aria-label": "Account status" },
              select: { native: true },
            }}
          >
            <option value="current">Current</option>
            <option value="active">Active</option>
            <option value="disabled">Disabled</option>
            <option value="deleted">Deleted</option>
            <option value="all">All statuses</option>
          </TextField>
        </Grid>
        <Grid size={{ xs: 12, md: 3 }} sx={{ textAlign: { md: "right" } }}>
          <Button
            variant="contained"
            startIcon={<Plus size={16} />}
            disabled={!data || query.isError}
            onClick={onCreate}
          >
            Create account
          </Button>
        </Grid>
      </Grid>
      <Divider />
      {query.isPending ? (
        <Typography role="status">Loading accounts...</Typography>
      ) : query.isError ? (
        <Alert severity="error">
          {message(query.error)}{" "}
          <Button type="submit" onClick={() => void query.refetch()}>
            Retry
          </Button>
        </Alert>
      ) : (
        <>
          <TableContainer>
            <Table
              size="small"
              aria-label="Accounts"
              sx={{ minWidth: 280, tableLayout: "fixed" }}
            >
              <TableHead>
                <TableRow>
                  <TableCell sx={{ width: "50%" }}>Account</TableCell>
                  <TableCell>Role</TableCell>
                  <TableCell>Status</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {data?.items.map((row) => {
                  const status = row.deleted_at
                    ? "Deleted"
                    : !row.is_active
                      ? "Disabled"
                      : row.password_ready
                        ? "Active"
                        : "Pending setup";
                  return (
                    <TableRow key={row.id} hover>
                      <TableCell sx={{ overflowWrap: "anywhere" }}>
                        <Link
                          href={listing.href(
                            `#accounts/${encodeURIComponent(row.id)}`,
                          )}
                        >
                          {row.display_name}
                        </Link>
                        <Typography
                          variant="body2"
                          color="text.secondary"
                          sx={{ display: { xs: "none", sm: "block" } }}
                        >
                          {row.username ?? "Username not set"}
                        </Typography>
                      </TableCell>
                      <TableCell sx={{ textTransform: "capitalize" }}>
                        {compact ? (
                          row.role
                        ) : (
                          <Chip
                            size="small"
                            label={row.role}
                            variant="outlined"
                          />
                        )}
                      </TableCell>
                      <TableCell>
                        {compact ? (
                          status
                        ) : (
                          <Chip
                            size="small"
                            variant="outlined"
                            color={
                              !row.deleted_at &&
                              row.is_active &&
                              row.password_ready
                                ? "success"
                                : "default"
                            }
                            label={status}
                          />
                        )}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </TableContainer>
          {!data?.items.length && (
            <Typography color="text.secondary">
              No matching accounts.
            </Typography>
          )}
        </>
      )}
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={2}
        sx={{ alignItems: { sm: "center" }, justifyContent: "flex-end" }}
      >
        {data && !query.isError && (
          <Typography
            variant="body2"
            color="text.secondary"
            sx={{ mr: "auto" }}
          >
            {data.items.length} accounts on this page
          </Typography>
        )}
        <TextField
          size="small"
          sx={{ width: { sm: 120 } }}
          value={listing.limit}
          onChange={(e) => listing.update({ limit: e.target.value })}
          label={"Rows per page"}
          select
          slotProps={{
            htmlInput: { "aria-label": "Rows per page" },
            select: { native: true },
          }}
        >
          {[20, 50, 100].map((size) => (
            <option key={size}>{size}</option>
          ))}
        </TextField>
        <Stack component="nav" direction="row" aria-label="Account pagination">
          <Tooltip title="Previous page">
            <span>
              <IconButton
                aria-label="Previous page"
                disabled={
                  query.isFetching || query.isError || !data?.previous_after
                }
                onClick={() =>
                  listing.update({ after: data?.previous_after ?? null })
                }
              >
                <ChevronLeft size={16} />
              </IconButton>
            </span>
          </Tooltip>
          <Tooltip title="Next page">
            <span>
              <IconButton
                aria-label="Next page"
                disabled={
                  query.isFetching || query.isError || !data?.next_before
                }
                onClick={() =>
                  listing.update({ before: data?.next_before ?? null })
                }
              >
                <ChevronRight size={16} />
              </IconButton>
            </span>
          </Tooltip>
        </Stack>
      </Stack>
    </Stack>
  );
}
