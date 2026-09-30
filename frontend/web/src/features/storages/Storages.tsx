import { IconButton, Button, Link, Table, TableBody, TableCell, TableContainer, TableHead, TableRow } from "@mui/material";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
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
    id = creating ? "" : decodeURIComponent(route.slice("storages/".length).replace(/\/edit$/, ""));
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
    return <main className="overview storages">
      {!canWrite ? <p role="alert">Write access required.</p>
        : !creating && detail.isPending ? <p role="status">Loading storage...</p>
        : !creating && detail.isError ? <p role="alert">{message(detail.error)}</p>
        : !creating && detail.data?.kind !== "s3" ? <p role="alert">Only S3 storage can be edited.</p>
        : <StorageEditor storage={creating ? undefined : detail.data} onClose={() => { window.location.hash = returnTo; }} onSaved={(saved) => { window.location.hash = listing.href(storageLink(saved)); }} />}
    </main>;
  }
  return (
    <main className="overview storages">
      {id && (
        <a className="back-link" href={listing.href("#storages")}>
          <ArrowLeft size={16} />
          Storage
        </a>
      )}
      <div className="page-heading">
        <div>
          <h1>{id || "Storage"}</h1>
        </div>
        <div className="page-actions">
          <IconButton type="submit"
            className="icon-button"
            title="Refresh"
            aria-label="Refresh"
            disabled={refreshing}
            onClick={() => void refreshStorages(cache)}
          >
            <RefreshCw size={17} className={refreshing ? "spin" : ""} />
          </IconButton>
          {canWrite &&
            (id ? (
              <>
                <IconButton type="submit"
                  className="icon-button"
                  title="Edit storage"
                  aria-label="Edit storage"
                  disabled={
                    !detail.data || detail.data.kind !== "s3" || current.isError
                  }
                  onClick={() => { window.location.hash = listing.href(`${storageLink(id)}/edit`); }}
                >
                  <Pencil size={17} />
                </IconButton>
                <IconButton type="submit"
                  className="icon-button danger"
                  color="error"
                  title="Delete storage"
                  aria-label="Delete storage"
                  disabled={!detail.data || current.isError}
                  onClick={() => setDialog("delete")}
                >
                  <Trash2 size={17} />
                </IconButton>
              </>
            ) : (
              <Button type="submit" variant="contained"
                className="primary action-button"
                onClick={() => { window.location.hash = listing.href("#storages/create/new"); }}
              >
                <Plus size={17} />
                Register
              </Button>
            ))}
        </div>
      </div>
      {current.isPending ? (
        <p className="empty" role="status">
          Loading storage...
        </p>
      ) : current.isError ? (
        <p className="query-error" role="alert">
          {message(current.error)}
        </p>
      ) : id && detail.data ? (
        <>
          {usage.isError && (
            <p className="query-error" role="alert">
              Unable to load usage. {message(usage.error)}
            </p>
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
            <p role="alert">Usage unavailable. {message(usage.error)}</p>
          )}
          <TableContainer><Table size="small" aria-label="Storage" sx={{ minWidth: 620 }}>
            <TableHead><TableRow><TableCell>Storage</TableCell><TableCell>Endpoint / Bucket</TableCell><TableCell align="right">Grove usage</TableCell><TableCell align="right">Configured capacity</TableCell></TableRow></TableHead>
            <TableBody>
            {page.rows.map((storage) => {
              const used = usage.isSuccess
                ? usage.data.find((row) => row.storage_id === storage.id)
                : undefined;
              return (
                <TableRow hover key={storage.id}>
                  <TableCell><Link href={listing.href(storageLink(storage.id))}>{storage.id}</Link></TableCell>
                  <TableCell>
                    {storage.kind === "fs"
                      ? storage.root_path
                      : storage.endpoint}
                    {storage.kind === "s3" && (
                      <div className="muted">{storage.bucket}</div>
                    )}
                  </TableCell>
                  <TableCell align="right">
                      {used
                        ? bytes(
                            used.active_bytes +
                              used.reserved_bytes +
                              used.purge_pending_bytes,
                          )
                        : "Unavailable"}
                  </TableCell>
                  <TableCell align="right">{bytes(storage.capacity_bytes)}</TableCell>
                </TableRow>
              );
            })}
            </TableBody>
          </Table></TableContainer>
          {page.total === 0 && (
              <p className="empty">
                {listing.search
                  ? "No matching storage."
                  : "No storage registered."}
              </p>
            )}
          <Pagination state={listing} {...page} />
        </>
      )}
      {canWrite && dialog === "delete" && (
        <DeleteStorage id={id} onClose={close} onReturnToList={showList} />
      )}
    </main>
  );
}
