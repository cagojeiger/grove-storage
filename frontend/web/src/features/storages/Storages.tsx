import {
  Alert,
  Box,
  Container,
  CircularProgress,
  IconButton,
  Button,
  Link,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { message, Usage } from "../../api/http";
import { command } from "../../api/commands";
import { storageLink } from "../../app/navigation";
import { bytes } from "../../design/format";
import { Storage, refreshStorages } from "./model";
import { StorageEditor } from "./StorageEditor";
import { DeleteStorage } from "./DeleteStorage";
import { StorageDetail } from "./StorageDetail";
import { TestConnection } from "./TestConnection";
import { paginate, useResourceList } from "../../app/resourceList";
import { ListToolbar, Pagination } from "../../design/ResourceList";

export function Storages({
  route,
  canWrite,
}: {
  route: string;
  canWrite: boolean;
}) {
  const cache = useQueryClient();
  const [dialog, setDialog] = useState<"delete" | null>(null);
  const listing = useResourceList();
  const creating = route === "storages/create/new";
  const editing = creating || /^storages\/[^/]+\/edit$/.test(route);
  let id = "";
  try {
    id = creating
      ? ""
      : decodeURIComponent(
          route.slice("storages/".length).replace(/\/edit$/, ""),
        );
  } catch {
    id = "";
  }
  const list = useQuery({
    queryKey: ["storages", "list"],
    enabled: !id && !creating,
    queryFn: ({ signal }) => command<Storage[]>("storage.list", {}, signal),
  });
  const detail = useQuery({
    queryKey: ["storages", "detail", id],
    enabled: Boolean(id),
    queryFn: ({ signal }) => command<Storage>("storage.show", { id }, signal),
  });
  const usage = useQuery({
    queryKey: ["storage-usage"],
    queryFn: ({ signal }) => command<Usage[]>("usage.storages", {}, signal),
  });
  const current = id ? detail : list;
  const refreshing = current.isFetching || usage.isFetching;
  const page = paginate(
    list.data ?? [],
    listing,
    (row) => row.id,
    (row) => `${row.id} ${row.endpoint ?? ""} ${row.bucket ?? ""}`,
  );
  function close() {
    setDialog(null);
  }
  function showList() {
    close();
    window.location.hash = listing.href("#storages");
  }
  if (editing) {
    const returnTo = listing.href(id ? storageLink(id) : "#storages");
    return (
      <Container component="main" maxWidth="lg" sx={{ py: 3 }}>
        {!canWrite ? (
          <Alert severity="error">Write access required.</Alert>
        ) : !creating && detail.isPending ? (
          <Typography role="status">Loading storage...</Typography>
        ) : !creating && detail.isError ? (
          <Alert severity="error">{message(detail.error)}</Alert>
        ) : !creating && detail.data?.kind !== "s3" ? (
          <Alert severity="error">Only S3 storage can be edited.</Alert>
        ) : (
          <StorageEditor
            storage={creating ? undefined : detail.data}
            onClose={() => {
              window.location.hash = returnTo;
            }}
            onSaved={(saved) => {
              window.location.hash = listing.href(storageLink(saved));
            }}
          />
        )}
      </Container>
    );
  }
  return (
    <Container component="main" maxWidth="lg" sx={{ py: 3 }}>
      <Stack spacing={3}>
        {id && (
          <Button
            component="a"
            href={listing.href("#storages")}
            startIcon={<ArrowLeft size={16} />}
            sx={{ alignSelf: "flex-start" }}
          >
            Storage
          </Button>
        )}
        <Stack
          direction="row"
          spacing={2}
          sx={{
            alignItems: "flex-start",
            justifyContent: "space-between",
            flexWrap: "wrap",
            rowGap: 1,
          }}
        >
          <Typography
            component="h1"
            variant="h5"
            sx={{ minWidth: 0, overflowWrap: "anywhere", flex: "1 1 180px" }}
          >
            {id || "Storage"}
          </Typography>
          <Stack direction="row" spacing={1}>
            <Tooltip title="Refresh">
              <span>
                <IconButton
                  type="button"
                  aria-label="Refresh"
                  disabled={refreshing}
                  onClick={() => void refreshStorages(cache)}
                >
                  {refreshing ? (
                    <CircularProgress size={18} color="inherit" />
                  ) : (
                    <RefreshCw size={18} />
                  )}
                </IconButton>
              </span>
            </Tooltip>
            {canWrite &&
              (id ? (
                <>
                  <Tooltip title="Edit storage">
                    <span>
                      <IconButton
                        type="button"
                        aria-label="Edit storage"
                        disabled={
                          !detail.data ||
                          detail.data.kind !== "s3" ||
                          current.isError
                        }
                        onClick={() => {
                          window.location.hash = listing.href(
                            `${storageLink(id)}/edit`,
                          );
                        }}
                      >
                        <Pencil size={17} />
                      </IconButton>
                    </span>
                  </Tooltip>
                  <Tooltip title="Delete storage">
                    <span>
                      <IconButton
                        type="button"
                        color="error"
                        aria-label="Delete storage"
                        disabled={!detail.data || current.isError}
                        onClick={() => setDialog("delete")}
                      >
                        <Trash2 size={17} />
                      </IconButton>
                    </span>
                  </Tooltip>
                </>
              ) : (
                <Button
                  type="button"
                  variant="contained"
                  startIcon={<Plus size={17} />}
                  onClick={() => {
                    window.location.hash = listing.href("#storages/create/new");
                  }}
                >
                  Register
                </Button>
              ))}
          </Stack>
        </Stack>
        {current.isPending ? (
          <Typography color="text.secondary" role="status" sx={{ py: 4 }}>
            Loading storage...
          </Typography>
        ) : current.isError ? (
          <Alert severity="error">{message(current.error)}</Alert>
        ) : id && detail.data ? (
          <>
            {usage.isError && (
              <Alert severity="error">
                Unable to load usage. {message(usage.error)}
              </Alert>
            )}
            <StorageDetail
              storage={detail.data}
              canWrite={canWrite}
              usage={
                usage.isError
                  ? undefined
                  : usage.data?.find((row) => row.storage_id === id)
              }
            />
            {detail.data.kind === "s3" && (
              <TestConnection
                key={`${id}:${detail.dataUpdatedAt}`}
                id={id}
                revision={detail.dataUpdatedAt}
                refreshing={detail.isFetching}
              />
            )}
          </>
        ) : (
          <>
            <ListToolbar state={listing} label="Search storage" />
            {usage.isError && (
              <Alert severity="error">
                Usage unavailable. {message(usage.error)}
              </Alert>
            )}
            <TableContainer>
              <Table
                size="small"
                aria-label="Storage"
                sx={{ tableLayout: "fixed" }}
              >
                <TableHead>
                  <TableRow>
                    <TableCell>Storage</TableCell>
                    <TableCell
                      sx={{ display: { xs: "none", md: "table-cell" } }}
                    >
                      Endpoint / Bucket
                    </TableCell>
                    <TableCell align="right">
                      Grove usage
                      <Box
                        component="span"
                        sx={{ display: { xs: "block", sm: "none" } }}
                      >
                        {" "}
                        / Capacity
                      </Box>
                    </TableCell>
                    <TableCell
                      align="right"
                      sx={{ display: { xs: "none", sm: "table-cell" } }}
                    >
                      Configured capacity
                    </TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {page.rows.map((storage) => {
                    const used = usage.isSuccess
                      ? usage.data.find((row) => row.storage_id === storage.id)
                      : undefined;
                    return (
                      <TableRow hover key={storage.id}>
                        <TableCell sx={{ overflowWrap: "anywhere" }}>
                          <Link href={listing.href(storageLink(storage.id))}>
                            {storage.id}
                          </Link>
                          <Typography
                            variant="body2"
                            color="text.secondary"
                            sx={{ display: { xs: "block", md: "none" } }}
                          >
                            {storage.kind === "s3"
                              ? storage.bucket
                              : storage.root_path}
                          </Typography>
                        </TableCell>
                        <TableCell
                          sx={{
                            display: { xs: "none", md: "table-cell" },
                            overflowWrap: "anywhere",
                          }}
                        >
                          {storage.kind === "fs"
                            ? storage.root_path
                            : storage.endpoint}
                          {storage.kind === "s3" && (
                            <Typography variant="body2" color="text.secondary">
                              {storage.bucket}
                            </Typography>
                          )}
                        </TableCell>
                        <TableCell
                          align="right"
                          sx={{ fontVariantNumeric: "tabular-nums" }}
                        >
                          {used
                            ? bytes(
                                used.active_bytes +
                                  used.reserved_bytes +
                                  used.purge_pending_bytes,
                              )
                            : "Unavailable"}
                          <Typography
                            variant="body2"
                            color="text.secondary"
                            sx={{ display: { xs: "block", sm: "none" } }}
                          >
                            {" "}
                            / {bytes(storage.capacity_bytes)}
                          </Typography>
                        </TableCell>
                        <TableCell
                          align="right"
                          sx={{
                            display: { xs: "none", sm: "table-cell" },
                            fontVariantNumeric: "tabular-nums",
                          }}
                        >
                          {bytes(storage.capacity_bytes)}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
            {page.total === 0 && (
              <Typography
                color="text.secondary"
                sx={{ py: 4, textAlign: "center" }}
              >
                {listing.search
                  ? "No matching storage."
                  : "No storage registered."}
              </Typography>
            )}
            <Pagination state={listing} {...page} />
          </>
        )}
      </Stack>
      {canWrite && dialog === "delete" && (
        <DeleteStorage id={id} onClose={close} onReturnToList={showList} />
      )}
    </Container>
  );
}
