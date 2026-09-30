import {
  TextField,
  Box,
  IconButton,
  Button,
  Link,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
} from "@mui/material";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Plus, Search } from "lucide-react";
import { identityRequest } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { isAccountPage, useAccountList } from "./accountList";

export function AccountsList({ onCreate }: { onCreate: () => void }) {
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
    <>
      <div className="list-toolbar resource-toolbar account-toolbar">
        <form
          onSubmit={(event) => {
            event.preventDefault();
            const value = new FormData(event.currentTarget).get("search");
            listing.update({
              q: typeof value === "string" ? value.trim() : "",
            });
          }}
        >
          <TextField
            label="Search accounts"
            key={listing.search}
            name="search"
            type="search"
            defaultValue={listing.search}
            slotProps={{
              htmlInput: { "aria-label": "Search accounts", maxLength: 80 },
            }}
          />
          <IconButton
            type="submit"
            className="icon-button"
            title="Search"
            aria-label="Search"
          >
            <Search size={18} />
          </IconButton>
        </form>
        <TextField
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
        <TextField
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
        <Button
          type="submit"
          variant="contained"
          className="primary"
          disabled={!data || query.isError}
          onClick={onCreate}
        >
          <Plus size={16} />
          Create user
        </Button>
      </div>
      <div className="account-list">
        {query.isPending ? (
          <p role="status">Loading accounts...</p>
        ) : query.isError ? (
          <p role="alert">
            {message(query.error)}{" "}
            <Button type="submit" onClick={() => void query.refetch()}>
              Retry
            </Button>
          </p>
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
                  {data?.items.map((row) => (
                    <TableRow key={row.id} hover>
                      <TableCell sx={{ overflowWrap: "anywhere" }}>
                        <Link
                          href={listing.href(
                            `#accounts/${encodeURIComponent(row.id)}`,
                          )}
                        >
                          {row.display_name}
                        </Link>
                        <Box
                          className="muted"
                          sx={{ display: { xs: "none", sm: "block" } }}
                        >
                          {row.id}
                        </Box>
                      </TableCell>
                      <TableCell sx={{ textTransform: "capitalize" }}>
                        {row.role}
                      </TableCell>
                      <TableCell>
                        {row.deleted_at
                          ? "Deleted"
                          : !row.is_active
                            ? "Disabled"
                            : row.password_ready
                              ? "Active"
                              : "Pending setup"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
            {!data?.items.length && (
              <p className="empty">No matching accounts.</p>
            )}
          </>
        )}
      </div>
      <div className="pagination">
        <TextField
          value={listing.limit}
          onChange={(e) => listing.update({ limit: e.target.value })}
          label={"Rows"}
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
        {data && !query.isError && (
          <span className="muted">
            {data.items.length} accounts on this page
          </span>
        )}
        <nav aria-label="Account pagination">
          <IconButton
            type="submit"
            className="icon-button"
            aria-label="Previous page"
            title="Previous page"
            disabled={
              query.isFetching || query.isError || !data?.previous_after
            }
            onClick={() =>
              listing.update({ after: data?.previous_after ?? null })
            }
          >
            <ChevronLeft size={16} />
          </IconButton>
          <IconButton
            type="submit"
            className="icon-button"
            aria-label="Next page"
            title="Next page"
            disabled={query.isFetching || query.isError || !data?.next_before}
            onClick={() =>
              listing.update({ before: data?.next_before ?? null })
            }
          >
            <ChevronRight size={16} />
          </IconButton>
        </nav>
      </div>
    </>
  );
}
