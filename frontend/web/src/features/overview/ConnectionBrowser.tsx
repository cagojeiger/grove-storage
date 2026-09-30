import {
  DialogContent,
  TextField,
  ButtonBase,
  IconButton,
  Typography,
} from "@mui/material";

import { useState } from "react";
import { useQueries } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, AppWindow, HardDrive } from "lucide-react";
import { command } from "../../api/commands";
import { Usage } from "../../api/http";
import { Dialog } from "../../design/Dialog";
import { bytes } from "../../design/format";
import { Client } from "../clients/model";
import { ClientTotals, sumClients } from "./connectionsModel";

export function ConnectionBrowser({
  kind,
  clients,
  storages,
  totals,
  onSelect,
  onClose,
}: {
  kind: "client" | "storage";
  clients: string[];
  storages: Usage[];
  totals?: Map<string, ClientTotals>;
  onSelect: (id: string) => void;
  onClose: () => void;
}) {
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(1);
  const ids = (
    kind === "client" ? clients : storages.map((row) => row.storage_id)
  ).filter((id) => id.toLowerCase().includes(search.toLowerCase()));
  const pages = Math.max(1, Math.ceil(ids.length / 20));
  const current = Math.min(page, pages);
  const visible = ids.slice((current - 1) * 20, current * 20);
  const details = useQueries({
    queries: (kind === "client" ? visible : []).map((id) => ({
      queryKey: ["clients", "detail", id],
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        command<Client>("client.show", { id }, signal),
      retry: false,
    })),
  });
  const groups = new Map<string, string[]>();
  visible.forEach((id, i) => {
    const query = details[i];
    const label =
      kind === "storage"
        ? "Storage"
        : query.isError
          ? "Assignment unavailable"
          : query.isSuccess
            ? query.data.storage_id
            : "Loading assignments...";
    groups.set(label, [...(groups.get(label) ?? []), id]);
  });
  return (
    <Dialog
      title={kind === "client" ? "More clients" : "More storage"}
      busy={false}
      onClose={onClose}
    >
      <DialogContent>
        <div className="connection-browser">
          <TextField
            className="connection-search"
            label={
              kind === "client"
                ? "Search hidden clients"
                : "Search hidden storage"
            }
            type="search"
            placeholder={
              kind === "client" ? "Search clients" : "Search storage"
            }
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              setPage(1);
            }}
          />
          {kind === "client" && (
            <p className="muted">Assignments on this page</p>
          )}
          {[...groups]
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([label, members]) => (
              <section key={label}>
                <Typography component="h3" variant="subtitle1">
                  {label} <span className="muted">({members.length})</span>
                </Typography>
                {members.map((id) => {
                  const summary = sumClients([id], totals);
                  const row = storages.find((row) => row.storage_id === id);
                  return (
                    <ButtonBase
                      type="submit"
                      className="connection-browser-row"
                      key={id}
                      onClick={() => onSelect(id)}
                      aria-label={`Show ${kind} ${id} on map`}
                    >
                      {kind === "client" ? (
                        <AppWindow size={16} />
                      ) : (
                        <HardDrive size={16} />
                      )}
                      <strong>{id}</strong>
                      <span>
                        {kind === "client"
                          ? summary
                            ? `${summary.files.toLocaleString("en-US")} files · ${bytes(summary.bytes)}`
                            : "Usage unavailable"
                          : row
                            ? `${row.active_files.toLocaleString("en-US")} files · ${bytes(row.active_bytes)} / ${bytes(row.capacity_bytes)}`
                            : "Usage unavailable"}
                      </span>
                    </ButtonBase>
                  );
                })}
              </section>
            ))}
          {!ids.length && (
            <p className="empty">
              No matching {kind === "client" ? "clients" : "storage"}.
            </p>
          )}
          <div className="connection-browser-pages">
            <span role="status">
              {ids.length ? (current - 1) * 20 + 1 : 0}–
              {Math.min(current * 20, ids.length)} of {ids.length}
            </span>
            <IconButton
              type="submit"
              className="icon-button"
              title="Previous page"
              aria-label="Previous page"
              disabled={current === 1}
              onClick={() => setPage(current - 1)}
            >
              <ChevronLeft size={16} />
            </IconButton>
            <IconButton
              type="submit"
              className="icon-button"
              title="Next page"
              aria-label="Next page"
              disabled={current === pages}
              onClick={() => setPage(current + 1)}
            >
              <ChevronRight size={16} />
            </IconButton>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
