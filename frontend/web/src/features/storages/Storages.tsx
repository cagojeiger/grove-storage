import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  ChevronRight,
  HardDrive,
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
  const [dialog, setDialog] = useState<"edit" | "delete" | null>(null);
  const listing = useResourceList();
  let id = "";
  try {
    id = decodeURIComponent(route.slice("storages/".length));
  } catch {
    id = "";
  }
  const list = useQuery({
    queryKey: ["storages", "list"],
    enabled: !id,
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
          <p className="eyebrow">REGISTRY</p>
          <h1>{id || "Storage"}</h1>
        </div>
        <div className="page-actions">
          <button
            className="icon-button"
            title="Refresh"
            aria-label="Refresh"
            disabled={refreshing}
            onClick={() => void refreshStorages(cache)}
          >
            <RefreshCw size={17} className={refreshing ? "spin" : ""} />
          </button>
          {canWrite &&
            (id ? (
              <>
                <button
                  className="icon-button"
                  title="Edit storage"
                  aria-label="Edit storage"
                  disabled={
                    !detail.data || detail.data.kind !== "s3" || current.isError
                  }
                  onClick={() => setDialog("edit")}
                >
                  <Pencil size={17} />
                </button>
                <button
                  className="icon-button danger"
                  title="Delete storage"
                  aria-label="Delete storage"
                  disabled={!detail.data || current.isError}
                  onClick={() => setDialog("delete")}
                >
                  <Trash2 size={17} />
                </button>
              </>
            ) : (
              <button
                className="primary action-button"
                onClick={() => setDialog("edit")}
              >
                <Plus size={17} />
                Register
              </button>
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
          <div className="registry-list capacity-registry">
            <div className="registry-labels" aria-hidden="true">
              <span>Storage</span>
              <span>Address</span>
              <span>Grove usage / Registered capacity</span>
              <span />
            </div>
            {page.rows.map((storage) => {
              const used = usage.isSuccess
                ? usage.data.find((row) => row.storage_id === storage.id)
                : undefined;
              return (
                <a
                  className="registry-row"
                  key={storage.id}
                  href={listing.href(storageLink(storage.id))}
                >
                  <div className="storage-name">
                    <HardDrive size={18} />
                    <div>
                      <h2>{storage.id}</h2>
                      <span className="muted">
                        {storage.kind === "fs" ? "Filesystem" : "S3"}
                      </span>
                    </div>
                  </div>
                  <span className="storage-address">
                    {storage.kind === "fs"
                      ? storage.root_path
                      : storage.endpoint}
                    {storage.kind === "s3" && (
                      <span className="muted">{storage.bucket}</span>
                    )}
                  </span>
                  <span className="storage-size">
                    <strong>
                      {used
                        ? bytes(
                            used.active_bytes +
                              used.reserved_bytes +
                              used.purge_pending_bytes,
                          )
                        : "Unavailable"}{" "}
                      / {bytes(storage.capacity_bytes)}
                    </strong>
                    {used && (
                      <>
                        <progress
                          aria-label={`${storage.id} usage`}
                          max={Math.max(1, storage.capacity_bytes)}
                          value={Math.max(
                            0,
                            used.active_bytes +
                              used.reserved_bytes +
                              used.purge_pending_bytes,
                          )}
                        />
                        <span className="capacity-breakdown">
                          <span>Active {bytes(used.active_bytes)}</span>
                          <span>Reserved {bytes(used.reserved_bytes)}</span>
                          <span>
                            Pending deletion {bytes(used.purge_pending_bytes)}
                          </span>
                          <span>Remaining {bytes(used.remaining_bytes)}</span>
                        </span>
                      </>
                    )}
                  </span>
                  <ChevronRight size={16} />
                </a>
              );
            })}
            {page.total === 0 && (
              <p className="empty">
                {listing.search
                  ? "No matching storage."
                  : "No storage registered."}
              </p>
            )}
          </div>
          <Pagination state={listing} {...page} />
        </>
      )}
      {canWrite && dialog === "edit" && (!id || detail.data?.kind === "s3") && (
        <StorageEditor
          storage={id ? detail.data : undefined}
          onClose={close}
          onSaved={(saved) => {
            close();
            window.location.hash = listing.href(storageLink(saved));
          }}
        />
      )}
      {canWrite && dialog === "delete" && (
        <DeleteStorage id={id} onClose={close} onReturnToList={showList} />
      )}
    </main>
  );
}
