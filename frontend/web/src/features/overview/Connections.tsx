import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowRight, ArrowUpRight, HardDrive, AppWindow } from "lucide-react";
import { command } from "../../api/commands";
import { Usage } from "../../api/http";
import { bytes } from "../../design/format";
import { storageLink } from "../../app/navigation";
import { Client, ClientUsage, clientLink, totals } from "../clients/model";

export function Connections({
  clients,
  storages,
  ready,
}: {
  clients: string[];
  storages: Usage[];
  ready: boolean;
}) {
  const [clientSearch, setClientSearch] = useState("");
  const [storageSearch, setStorageSearch] = useState("");
  const [selected, setSelected] = useState("");
  const usage = useQuery({
    queryKey: ["clients", "usage"],
    enabled: clients.length > 0,
    queryFn: ({ signal }) =>
      command<ClientUsage[]>("usage.clients", {}, signal),
  });
  const assignment = useQuery({
    queryKey: ["clients", "detail", selected],
    enabled: Boolean(selected),
    queryFn: ({ signal }) =>
      command<Client>("client.show", { id: selected }, signal),
  });
  const assigned = assignment.isSuccess
    ? assignment.data.storage_id
    : undefined;
  const clientRows = clients
    .filter((id) => id.toLowerCase().includes(clientSearch.toLowerCase()))
    .sort((a, b) => a.localeCompare(b))
    .slice(0, 5);
  const matches = storages
    .filter((row) =>
      row.storage_id.toLowerCase().includes(storageSearch.toLowerCase()),
    )
    .sort((a, b) => a.storage_id.localeCompare(b.storage_id));
  const target = storages.find((row) => row.storage_id === assigned);
  const storageRows = target
    ? [target, ...matches.filter((row) => row !== target)].slice(0, 5)
    : matches.slice(0, 5);
  return (
    <section className="connections" aria-label="Storage connections">
      <div className="connection-column">
        <div className="section-heading">
          <h2>Clients</h2>
          <span className="muted">
            {clientRows.length} / {clients.length}
          </span>
        </div>
        <label className="connection-search">
          <span className="sr-only">Find client</span>
          <input
            type="search"
            placeholder="Find client"
            value={clientSearch}
            onChange={(e) => {
              setClientSearch(e.target.value);
              setSelected("");
            }}
          />
        </label>
        <div className="connection-items" role="region" aria-label="Client connections" tabIndex={0}>
          {clientRows.map((id) => {
            const summary = usage.isSuccess
              ? totals(usage.data, id)
              : undefined;
            return (
              <article
                className={`connection-item ${selected === id ? "selected" : ""}`}
                key={id}
              >
                <button
                  className="connection-select"
                  aria-pressed={selected === id}
                  aria-label={`Select client ${id}`}
                  onClick={() => setSelected(selected === id ? "" : id)}
                >
                  <AppWindow size={18} />
                  <strong>{id}</strong>
                </button>
                <p className="muted">
                  {summary
                    ? `${summary.files.toLocaleString("en-US")} files · ${bytes(summary.bytes)}`
                    : "Usage unavailable"}
                </p>
                <a className="connection-open" href={clientLink(id)}>
                  Open client <ArrowUpRight size={14} />
                </a>
              </article>
            );
          })}
        </div>
        {!clientRows.length && (
          <p className="empty">
            {clients.length ? "No matching clients." : "No clients registered."}
          </p>
        )}
        <a className="connection-all" href="#clients">
          View all clients <ArrowRight size={16} />
        </a>
      </div>
      <div className="grove-hub">
        <ArrowRight className="flow-arrow" size={22} aria-hidden="true" />
        <div className="hub-label">
          <img
            src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
            alt=""
          />
          <h2>Grove Storage</h2>
          <span className="muted">S3 gateway</span>
          <p>
            <span className={`dot ${ready ? "online" : ""}`} />
            API {ready ? "ready" : "unavailable"}
          </p>
        </div>
        <ArrowRight className="flow-arrow" size={22} aria-hidden="true" />
        {selected && (
          <p className="assignment" role="status">
            {assignment.isError
              ? "Storage assignment unavailable"
              : assigned
                ? `${selected} → ${assigned}`
                : "Loading storage assignment..."}
          </p>
        )}
      </div>
      <div className="connection-column">
        <div className="section-heading">
          <h2>Storage</h2>
          <span className="muted">
            {storageRows.length} / {storages.length}
          </span>
        </div>
        <label className="connection-search">
          <span className="sr-only">Find storage</span>
          <input
            type="search"
            placeholder="Find storage"
            value={storageSearch}
            onChange={(e) => {
              setStorageSearch(e.target.value);
              setSelected("");
            }}
          />
        </label>
        <div className="connection-items" role="region" aria-label="Registered storage connections" tabIndex={0}>
          {storageRows.map((row) => (
            <article
              className={`connection-item ${row.storage_id === assigned ? "selected" : ""}`}
              key={row.storage_id}
            >
              <a
                className="connection-title"
                href={storageLink(row.storage_id)}
              >
                <HardDrive size={18} />
                <h3>{row.storage_id}</h3>
                <ArrowUpRight size={14} />
              </a>
              <p className="muted">
                {row.kind.toUpperCase()} · Active files:{" "}
                {row.active_files.toLocaleString("en-US")}
              </p>
              <strong>
                {bytes(
                  row.active_bytes +
                    row.reserved_bytes +
                    row.purge_pending_bytes,
                )}{" "}
                / {bytes(row.capacity_bytes)}
              </strong>
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
              <div className="capacity-breakdown">
                <span>Active {bytes(row.active_bytes)}</span>
                <span>Remaining {bytes(row.remaining_bytes)}</span>
              </div>
              <span className="muted">
                Registered capacity · Connection not checked
              </span>
            </article>
          ))}
        </div>
        {!storageRows.length && (
          <p className="empty">
            {storages.length
              ? "No matching storage."
              : "No storage registered."}
          </p>
        )}
        <a className="connection-all" href="#storages">
          View all storage <ArrowRight size={16} />
        </a>
      </div>
    </section>
  );
}
