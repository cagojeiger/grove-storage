import { useQuery } from "@tanstack/react-query";
import { RefreshCw, HardDrive, History } from "lucide-react";
import { message, request, Usage } from "../../api/http";
import { command } from "../../api/commands";
import { bytes } from "../../design/format";

export function Overview() {
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
    key: "active_bytes" | "reserved_bytes" | "purge_pending_bytes",
  ) => data?.usage.reduce((total, row) => total + row[key], 0) ?? 0;
  return (
    <main className="overview">
      <div className="page-heading">
        <div>
          <p className="eyebrow">WORKSPACE</p>
          <h1>Overview</h1>
        </div>
        <button
          className="icon-button"
          title="Refresh"
          aria-label="Refresh"
          onClick={() => void query.refetch()}
          disabled={query.isFetching}
        >
          <RefreshCw size={18} className={query.isFetching ? "spin" : ""} />
        </button>
      </div>
      {query.isPending ? (
        <p role="status">Loading...</p>
      ) : query.isError ? (
        <p role="alert">{message(query.error)}</p>
      ) : (
        data && (
          <>
            <div className="status-line">
              <span className={`dot ${data.ready ? "online" : ""}`} />
              <strong>
                API {data.ready ? "ready" : "readiness check failed"}
              </strong>
              <span className="muted">Storage connectivity not checked</span>
            </div>
            <dl className="metrics">
              <div>
                <dt>Storage</dt>
                <dd>{data.usage.length}</dd>
              </div>
              <div>
                <dt>Clients</dt>
                <dd>{data.clients.length}</dd>
              </div>
              <div>
                <dt>Active data</dt>
                <dd>{bytes(sum("active_bytes"))}</dd>
              </div>
              <div>
                <dt>Reserved / Pending deletion</dt>
                <dd className="compact-value">
                  {bytes(sum("reserved_bytes"))} /{" "}
                  {bytes(sum("purge_pending_bytes"))}
                </dd>
              </div>
            </dl>
            <section>
              <div className="section-heading">
                <h2>Storage usage</h2>
                <a className="back-link" href="#usage"><History size={16} />Usage history</a>
              </div>
              {data.usage.length === 0 ? (
                <div className="empty">
                  <HardDrive size={28} aria-hidden="true" />
                  <p>No storage registered.</p>
                </div>
              ) : (
                <div className="storage-list">
                  {data.usage.map((row) => (
                    <article className="storage-row" key={row.storage_id}>
                      <div className="storage-name">
                        <HardDrive size={18} aria-hidden="true" />
                        <div>
                          <h3>{row.storage_id}</h3>
                          <span className="muted">
                            {row.kind.toUpperCase()} · Active files:{" "}
                            {row.active_files.toLocaleString("en-US")}
                          </span>
                        </div>
                      </div>
                      <dl className="storage-values">
                        <div>
                          <dt>Active</dt>
                          <dd>{bytes(row.active_bytes)}</dd>
                        </div>
                        <div>
                          <dt>Reserved</dt>
                          <dd>{bytes(row.reserved_bytes)}</dd>
                        </div>
                        <div>
                          <dt>Pending deletion</dt>
                          <dd>{bytes(row.purge_pending_bytes)}</dd>
                        </div>
                        <div>
                          <dt>Remaining</dt>
                          <dd
                            className={row.remaining_bytes < 0 ? "danger" : ""}
                          >
                            {bytes(row.remaining_bytes)}
                          </dd>
                        </div>
                      </dl>
                      <div className="capacity">
                        <progress
                          aria-label={`${row.storage_id} usage`}
                          max={Math.max(1, row.capacity_bytes)}
                          value={Math.max(
                            0,
                            row.active_bytes +
                              row.reserved_bytes +
                              row.purge_pending_bytes,
                          )}
                        />
                        <span className="muted">
                          Registered capacity {bytes(row.capacity_bytes)}
                        </span>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>
          </>
        )
      )}
    </main>
  );
}
