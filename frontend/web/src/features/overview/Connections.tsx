import { ButtonBase, Typography } from "@mui/material";
import { useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowRight, HardDrive, AppWindow, Layers } from "lucide-react";
import { command } from "../../api/commands";
import { Usage } from "../../api/http";
import { bytes } from "../../design/format";
import { storageLink } from "../../app/navigation";
import { Client, ClientUsage, clientLink } from "../clients/model";
import { clientTotals, foldConnections, sumClients } from "./connectionsModel";
import { ConnectionPaths } from "./ConnectionPaths";
import { ConnectionBrowser } from "./ConnectionBrowser";

export function Connections({
  clients,
  storages,
  ready,
}: {
  clients: string[];
  storages: Usage[];
  ready: boolean;
}) {
  const graph = useRef<HTMLElement>(null);
  const [selected, setSelected] = useState("");
  const [selectedStorage, setSelectedStorage] = useState("");
  const [browser, setBrowser] = useState<"client" | "storage" | null>(null);
  const selectedClient = clients.includes(selected) ? selected : "";
  const usage = useQuery({
    queryKey: ["clients", "usage"],
    enabled: clients.length > 0,
    queryFn: ({ signal }) =>
      command<ClientUsage[]>("usage.clients", {}, signal),
  });
  const totals = useMemo(
    () => (usage.isSuccess ? clientTotals(usage.data) : undefined),
    [usage.isSuccess, usage.data],
  );
  const assignment = useQuery({
    queryKey: ["clients", "detail", selectedClient],
    enabled: Boolean(selectedClient),
    queryFn: ({ signal }) =>
      command<Client>("client.show", { id: selectedClient }, signal),
    retry: false,
  });
  const assigned =
    selectedClient && assignment.isSuccess
      ? assignment.data.storage_id
      : undefined;
  const activeStorage = selectedClient ? assigned : selectedStorage;
  const clientRows = foldConnections(clients, (id) => id, selectedClient);
  const storageRows = foldConnections(
    storages,
    (row) => row.storage_id,
    activeStorage,
  );
  const hiddenClients = sumClients(clientRows.hidden, totals);
  const hiddenStorage = storageRows.hidden.reduce(
    (sum, row) => ({
      files: sum.files + row.active_files,
      bytes: sum.bytes + row.active_bytes,
      capacity: sum.capacity + row.capacity_bytes,
    }),
    { files: 0, bytes: 0, capacity: 0 },
  );
  const revision = JSON.stringify([
    clientRows.visible,
    storageRows.visible.map((row) => row.storage_id),
    selectedClient,
    activeStorage,
    clientRows.hidden.length,
    storageRows.hidden.length,
  ]);
  function chooseClient(id: string) {
    setSelected(id);
    setSelectedStorage("");
  }
  function chooseStorage(id: string) {
    setSelected("");
    setSelectedStorage(id);
  }
  return (
    <>
      <div className="connections-heading">
        <Typography component="h2" variant="h6">
          Connections
        </Typography>
        <span className="muted">Configured routes</span>
      </div>
      <section
        className="connections"
        aria-label="Storage connections"
        ref={graph}
      >
        <ConnectionPaths container={graph} revision={revision} />
        <div className="connection-column">
          <div className="section-heading">
            <Typography component="h2" variant="h6">
              Clients
            </Typography>
            <span className="muted">{clients.length}</span>
          </div>
          <div
            className="connection-items"
            role="region"
            aria-label="Client connections"
          >
            {clientRows.visible.map((id) => {
              const summary = sumClients([id], totals);
              return (
                <article
                  className={`connection-item ${selectedClient === id ? "selected" : ""}`}
                  key={id}
                  data-connection={`client:${id}`}
                  data-side="client"
                  data-selected={selectedClient === id}
                >
                  <ButtonBase
                    component="a"
                    href={clientLink(id)}
                    className="connection-select"
                    aria-label={`Open client ${id}`}
                    onMouseEnter={() => chooseClient(id)}
                    onFocus={() => chooseClient(id)}
                  >
                    <AppWindow size={18} />
                    <strong>{id}</strong>
                  </ButtonBase>
                  <p className="connection-usage">
                    {summary ? (
                      <>
                        <span>
                          {summary.files.toLocaleString("en-US")} files
                        </span>
                        <strong>{bytes(summary.bytes)}</strong>
                      </>
                    ) : (
                      <span className="muted">Usage unavailable</span>
                    )}
                  </p>
                </article>
              );
            })}
            {clientRows.hidden.length > 0 && (
              <ButtonBase
                type="submit"
                className="connection-item connection-more"
                data-connection="client:more"
                data-side="client"
                aria-label={`Show ${clientRows.hidden.length} more clients`}
                aria-haspopup="dialog"
                onClick={() => setBrowser("client")}
              >
                <strong>
                  <Layers size={16} /> + {clientRows.hidden.length} clients
                </strong>
                <span>
                  {hiddenClients
                    ? `${hiddenClients.files.toLocaleString("en-US")} files · ${bytes(hiddenClients.bytes)}`
                    : "Usage unavailable"}
                </span>
              </ButtonBase>
            )}
          </div>
          {!clients.length && <p className="empty">No clients registered.</p>}
          <a className="connection-all" href="#clients">
            View all clients <ArrowRight size={16} />
          </a>
        </div>
        <div className="grove-hub">
          <div className="hub-label">
            <img
              src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
              alt=""
            />
            <Typography component="h2" variant="h6">
              Grove Storage
            </Typography>
            <span className="muted">S3 gateway</span>
            <p>
              <span className={`dot ${ready ? "online" : ""}`} />
              API {ready ? "ready" : "unavailable"}
            </p>
          </div>
        </div>
        <div className="connection-column">
          <div className="section-heading">
            <Typography component="h2" variant="h6">
              Storage
            </Typography>
            <span className="muted">{storages.length}</span>
          </div>
          <div
            className="connection-items"
            role="region"
            aria-label="Registered storage connections"
          >
            {storageRows.visible.map((row) => (
              <article
                className={`connection-item ${row.storage_id === activeStorage ? "selected" : ""}`}
                key={row.storage_id}
                data-connection={`storage:${row.storage_id}`}
                data-side="storage"
                data-selected={row.storage_id === activeStorage}
              >
                <div className="connection-storage-heading">
                  <ButtonBase
                    component="a"
                    href={storageLink(row.storage_id)}
                    className="connection-select connection-title"
                    aria-label={`Open storage ${row.storage_id}`}
                    onMouseEnter={() => chooseStorage(row.storage_id)}
                    onFocus={() => chooseStorage(row.storage_id)}
                  >
                    <HardDrive size={18} />
                    <strong>{row.storage_id}</strong>
                  </ButtonBase>
                </div>
                <p className="muted">{row.kind.toUpperCase()} · Configured</p>
              </article>
            ))}
            {storageRows.hidden.length > 0 && (
              <ButtonBase
                type="submit"
                className="connection-item connection-more"
                data-connection="storage:more"
                data-side="storage"
                aria-label={`Show ${storageRows.hidden.length} more storage`}
                aria-haspopup="dialog"
                onClick={() => setBrowser("storage")}
              >
                <strong>
                  <Layers size={16} /> + {storageRows.hidden.length} storage
                </strong>
                <span>
                  {hiddenStorage.files.toLocaleString("en-US")} files ·{" "}
                  {bytes(hiddenStorage.bytes)} active
                </span>
                <span>{bytes(hiddenStorage.capacity)} registered capacity</span>
              </ButtonBase>
            )}
          </div>
          {!storages.length && <p className="empty">No storage registered.</p>}
        </div>
      </section>
      <div className="connection-selection" role="status">
        {selectedClient
          ? assignment.isError
            ? "Storage assignment unavailable"
            : assigned
              ? `${selectedClient} → ${assigned}${storages.some((row) => row.storage_id === assigned) ? "" : " · Storage unavailable"}`
              : "Loading storage assignment..."
          : activeStorage &&
              storages.some((row) => row.storage_id === activeStorage)
            ? activeStorage
            : "Configured connections · Storage connectivity not checked"}
      </div>
      {browser && (
        <ConnectionBrowser
          kind={browser}
          clients={clientRows.hidden}
          storages={storageRows.hidden}
          totals={totals}
          onClose={() => setBrowser(null)}
          onSelect={(id) => {
            if (browser === "client") chooseClient(id);
            else chooseStorage(id);
            setBrowser(null);
          }}
        />
      )}
    </>
  );
}
