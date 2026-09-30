import {
  Alert,
  Container,
  IconButton,
  Button,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { message, Usage } from "../../api/http";
import { command } from "../../api/commands";
import { storageLink } from "../../app/navigation";
import { Page } from "../../app/Page";
import { Storage, refreshStorages } from "./model";
import { StorageEditor } from "./StorageEditor";
import { DeleteStorage } from "./DeleteStorage";
import { StorageDetail } from "./StorageDetail";
import { StorageGrid } from "./StorageGrid";
import { TestConnection } from "./TestConnection";
import { paginate, useResourceList } from "../../app/resourceList";

export function Storages({
  route,
  canWrite,
}: {
  route: string;
  canWrite: boolean;
}) {
  const cache = useQueryClient();
  const [deleting, setDeleting] = useState(false);
  const listing = useResourceList();
  const creating = route === "storages/create/new";
  const editing = creating || /^storages\/[^/]+\/edit$/.test(route);
  let id = "";
  try {
    if (!creating && route.startsWith("storages/")) {
      id = decodeURIComponent(
        route.slice("storages/".length).replace(/\/edit$/, ""),
      );
    }
  } catch {
    /* Invalid resource links return to the list. */
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
  const page = paginate(
    list.data ?? [],
    listing,
    (row) => row.id,
    (row) => `${row.id} ${row.endpoint ?? ""} ${row.bucket ?? ""}`,
  );
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
    <>
      <Page
        title={id || "Storage"}
        back={
          id ? { label: "Storage", href: listing.href("#storages") } : undefined
        }
        actions={
          <>
            <Tooltip title="Refresh">
              <span>
                <IconButton
                  aria-label="Refresh"
                  disabled={current.isFetching || usage.isFetching}
                  onClick={() => void refreshStorages(cache)}
                >
                  <RefreshCw size={20} />
                </IconButton>
              </span>
            </Tooltip>
            {canWrite &&
              (id ? (
                <>
                  <Tooltip title="Edit storage">
                    <span>
                      <IconButton
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
                        <Pencil size={20} />
                      </IconButton>
                    </span>
                  </Tooltip>
                  <Tooltip title="Delete storage">
                    <span>
                      <IconButton
                        color="error"
                        aria-label="Delete storage"
                        disabled={!detail.data || current.isError}
                        onClick={() => setDeleting(true)}
                      >
                        <Trash2 size={20} />
                      </IconButton>
                    </span>
                  </Tooltip>
                </>
              ) : (
                <Button
                  variant="contained"
                  startIcon={<Plus size={20} />}
                  onClick={() => {
                    window.location.hash = listing.href("#storages/create/new");
                  }}
                >
                  Register
                </Button>
              ))}
          </>
        }
      >
        {current.isPending ? (
          <Typography role="status">Loading storage...</Typography>
        ) : current.isError ? (
          <Alert
            severity="error"
            action={
              <Button color="inherit" onClick={() => void current.refetch()}>
                Retry
              </Button>
            }
          >
            {message(current.error)}
          </Alert>
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
            <TextField
              label="Search storage"
              type="search"
              value={listing.search}
              onChange={(event) =>
                listing.update({ search: event.target.value })
              }
              sx={{ maxWidth: 360 }}
            />
            {usage.isError && (
              <Alert severity="error">
                Usage unavailable. {message(usage.error)}
              </Alert>
            )}
            <StorageGrid
              rows={page.rows}
              usage={usage.isSuccess ? usage.data : undefined}
              state={listing}
              page={page.page}
              total={page.total}
            />
          </>
        )}
      </Page>
      {canWrite && deleting && (
        <DeleteStorage
          id={id}
          onClose={() => setDeleting(false)}
          onReturnToList={() => {
            setDeleting(false);
            window.location.hash = listing.href("#storages");
          }}
        />
      )}
    </>
  );
}
