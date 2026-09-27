import { useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw, History } from "lucide-react";
import { message, request, Usage } from "../../api/http";
import { command } from "../../api/commands";
import { bytes } from "../../design/format";
import { Connections } from "./Connections";

export function Overview() {
  const cache = useQueryClient();
  const query = useQuery({
    queryKey: ["overview"],
    queryFn: async ({ signal }) => {
      const [usage, clients, ready] = await Promise.all([
        command<Usage[]>("usage.storages", {}, signal),
        command<string[]>("client.list", {}, signal),
        request<{ status: string }>("/readyz", { signal }).then(
          (value) => value.status === "ready",
          () => false,
        ),
      ]);
      return { usage, clients, ready };
    },
    retry: false,
  });
  const data = query.data;
  const sum = (
    key:
      | "active_bytes"
      | "active_files"
      | "capacity_bytes"
      | "remaining_bytes"
      | "reserved_bytes"
      | "purge_pending_bytes",
  ) => data?.usage.reduce((total, row) => total + row[key], 0) ?? 0;
  return (
    <main className="overview topology-overview">
      <div className="page-heading">
        <div>
          <p className="eyebrow">WORKSPACE</p>
          <h1>Overview</h1>
        </div>
        <div className="page-actions">
          <a className="back-link" href="#usage">
            <History size={16} />
            Usage history
          </a>
          <button
            className="icon-button"
            title="Refresh"
            aria-label="Refresh"
            disabled={query.isFetching}
            onClick={() => {
              void query.refetch();
              void cache.invalidateQueries({ queryKey: ["clients"] });
            }}
          >
            <RefreshCw size={18} className={query.isFetching ? "spin" : ""} />
          </button>
        </div>
      </div>
      {query.isPending ? (
        <p role="status">Loading...</p>
      ) : query.isError ? (
        <p role="alert">{message(query.error)}</p>
      ) : (
        data && (
          <>
            <dl className="metrics">
              <div>
                <dt>Clients</dt>
                <dd>{data.clients.length}</dd>
              </div>
              <div>
                <dt>Storage</dt>
                <dd>{data.usage.length}</dd>
              </div>
              <div>
                <dt>Total files</dt>
                <dd>{sum("active_files").toLocaleString("en-US")}</dd>
              </div>
              <div>
                <dt>Total stored</dt>
                <dd>{bytes(sum("active_bytes"))}</dd>
              </div>
              <div>
                <dt>Registered capacity</dt>
                <dd>{bytes(sum("capacity_bytes"))}</dd>
              </div>
            </dl>
            <div className="overview-accounting muted">
              <span>Reserved {bytes(sum("reserved_bytes"))}</span>
              <span>Pending deletion {bytes(sum("purge_pending_bytes"))}</span>
              <span>Remaining {bytes(sum("remaining_bytes"))}</span>
            </div>
            <Connections
              clients={data.clients}
              storages={data.usage}
              ready={data.ready}
            />
          </>
        )
      )}
    </main>
  );
}
