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

export function Storages({ route, canWrite }: { route: string; canWrite: boolean }) {
  const cache = useQueryClient();
  const [dialog, setDialog] = useState<"edit" | "delete" | null>(null);
  const [search, setSearch] = useState("");
  let id = "";
  try {
    id = decodeURIComponent(route.slice("storages/".length));
  } catch {
    id = "";
  }
  const list = useQuery({
    queryKey: ["storages", "list"],
    enabled: !id,
    queryFn: ({ signal }) =>
      command<Storage[]>("storage.list", {}, signal),
  });
  const detail = useQuery({
    queryKey: ["storages", "detail", id],
    enabled: Boolean(id),
    queryFn: ({ signal }) =>
      command<Storage>("storage.show", { id }, signal),
  });
  const usage = useQuery({
    queryKey: ["storage-usage"],
    queryFn: ({ signal }) => command<Usage[]>("usage.storages", {}, signal),
  });
  const current = id ? detail : list;
  const refreshing = current.isFetching || usage.isFetching;
  const rows = list.data?.filter((storage) =>
    storage.id.toLowerCase().includes(search.toLowerCase()),
  );
  function close() {
    setDialog(null);
  }
  function showList() {
    close();
    window.location.hash = "storages";
  }
  return (
    <main className="overview storages">
      {id && (
        <a className="back-link" href="#storages">
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
          {canWrite && (id ? (
            <>
              <button
                className="icon-button"
                title="Edit storage"
                aria-label="Edit storage"
                disabled={!detail.data || current.isError}
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
        </>
      ) : (
        <>
          <div className="list-toolbar">
            <label>
              <span className="sr-only">Search storage</span>
              <input
                type="search"
                placeholder="Search storage"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
              />
            </label>
            <span className="muted">{(list.data?.length ?? 0).toLocaleString("en-US")}</span>
          </div>
          <div className="registry-list">
            <div className="registry-labels" aria-hidden="true">
              <span>Storage</span>
              <span>Address</span>
              <span>Registered capacity</span>
              <span />
            </div>
            {rows?.map((storage) => (
              <a
                className="registry-row"
                key={storage.id}
                href={storageLink(storage.id)}
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
                  {storage.kind === "fs" ? storage.root_path : storage.endpoint}
                  {storage.kind === "s3" && (
                    <span className="muted">{storage.bucket}</span>
                  )}
                </span>
                <span className="storage-size">
                  {bytes(storage.capacity_bytes)}
                </span>
                <ChevronRight size={16} />
              </a>
            ))}
            {rows?.length === 0 && (
              <p className="empty">
                {search ? "No matching storage." : "No storage registered."}
              </p>
            )}
          </div>
        </>
      )}
      {canWrite && dialog === "edit" && (
        <StorageEditor
          storage={id ? detail.data : undefined}
          onClose={close}
          onSaved={(saved) => {
            close();
            window.location.hash = storageLink(saved);
          }}
        />
      )}
      {canWrite && dialog === "delete" && (
        <DeleteStorage id={id} onClose={close} onReturnToList={showList} />
      )}
    </main>
  );
}
