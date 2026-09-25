import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, ChevronRight, Plus, RefreshCw, Trash2 } from "lucide-react";
import { command } from "../../api/commands";
import { message } from "../../api/http";
import { bytes } from "../../design/format";
import { storageLink } from "../../app/navigation";
import {
  Client,
  ClientUsage,
  clientLink,
  refreshClients,
  totals,
} from "./model";
import { ClientDialog } from "./ClientDialog";
import { ClientKeys } from "./ClientKeys";

export function Clients({
  route,
  canWrite,
}: {
  route: string;
  canWrite: boolean;
}) {
  const cache = useQueryClient();
  const [dialog, setDialog] = useState(false);
  const [search, setSearch] = useState("");
  let id = "";
  try {
    if (route.startsWith("clients/")) id = decodeURIComponent(route.slice(8));
  } catch {
    /* Invalid links fall back to the list. */
  }
  const list = useQuery({
    queryKey: ["clients", "list"],
    enabled: !id,
    queryFn: ({ signal }) => command<string[]>("client.list", {}, signal),
  });
  const detail = useQuery({
    queryKey: ["clients", "detail", id],
    enabled: Boolean(id),
    queryFn: ({ signal }) => command<Client>("client.show", { id }, signal),
  });
  const usage = useQuery({
    queryKey: ["clients", "usage"],
    queryFn: ({ signal }) =>
      command<ClientUsage[]>("usage.clients", {}, signal),
  });
  const current = id ? detail : list;
  const used = usage.isSuccess ? totals(usage.data, id) : undefined;
  return (
    <main className="overview storages clients">
      {id && (
        <a className="back-link" href="#clients">
          <ArrowLeft size={16} />
          Clients
        </a>
      )}
      <div className="page-heading">
        <div>
          <p className="eyebrow">REGISTRY</p>
          <h1>{id || "Clients"}</h1>
        </div>
        <div className="page-actions">
          <button
            className="icon-button"
            title="Refresh clients"
            aria-label="Refresh clients"
            disabled={current.isFetching || usage.isFetching}
            onClick={() => void refreshClients(cache)}
          >
            <RefreshCw size={18} />
          </button>
          {canWrite &&
            (id ? (
              <button
                className="icon-button danger"
                title="Delete client"
                aria-label="Delete client"
                disabled={!detail.data || current.isError}
                onClick={() => setDialog(true)}
              >
                <Trash2 size={18} />
              </button>
            ) : (
              <button className="primary" onClick={() => setDialog(true)}>
                <Plus size={18} />
                Create client
              </button>
            ))}
        </div>
      </div>
      {current.isPending ? (
        <p role="status">Loading clients...</p>
      ) : current.isError ? (
        <p role="alert">{message(current.error)}</p>
      ) : id && detail.data ? (
        <>
          <dl className="detail-fields">
            <div>
              <dt>Client ID</dt>
              <dd>{id}</dd>
            </div>
            <div>
              <dt>Storage</dt>
              <dd>
                <a href={storageLink(detail.data.storage_id)}>
                  {detail.data.storage_id}
                </a>
              </dd>
            </div>
            <div>
              <dt>Active files</dt>
              <dd>
                {usage.isError || !used
                  ? "Unavailable"
                  : used.files.toLocaleString("en-US")}
              </dd>
            </div>
            <div>
              <dt>Active data</dt>
              <dd>
                {usage.isError || !used ? "Unavailable" : bytes(used.bytes)}
              </dd>
            </div>
          </dl>
          {canWrite && <ClientKeys key={id} clientId={id} />}
        </>
      ) : (
        <>
          <div className="list-toolbar">
            <label>
              <span className="sr-only">Search clients</span>
              <input
                type="search"
                placeholder="Search clients"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </label>
            <span className="muted">{list.data?.length ?? 0}</span>
          </div>
          <div className="account-list">
            {list.data
              ?.filter((row) =>
                row.toLowerCase().includes(search.toLowerCase()),
              )
              .map((row) => {
                const summary = usage.isSuccess
                  ? totals(usage.data, row)
                  : undefined;
                return (
                  <a className="account-row" key={row} href={clientLink(row)}>
                    <strong>{row}</strong>
                    <span>
                      {summary
                        ? `${summary.files.toLocaleString("en-US")} files`
                        : "Unavailable"}
                    </span>
                    <span>
                      {summary ? bytes(summary.bytes) : "Unavailable"}
                    </span>
                    <ChevronRight size={16} />
                  </a>
                );
              })}
          </div>
          {list.data?.length === 0 && (
            <p className="empty">No clients registered.</p>
          )}
        </>
      )}
      {usage.isError && (
        <p role="alert">Usage unavailable. {message(usage.error)}</p>
      )}
      {canWrite && dialog && (
        <ClientDialog
          id={id || undefined}
          onClose={() => setDialog(false)}
          onSaved={(target) => {
            setDialog(false);
            location.hash = target ? clientLink(target) : "clients";
          }}
        />
      )}
    </main>
  );
}
