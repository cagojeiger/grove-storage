import {
  DialogContent,
  TextField,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Stack,
  TablePagination,
  Typography,
  Dialog,
  DialogTitle,
  useMediaQuery,
  useTheme,
  DialogActions,
  Button,
} from "@mui/material";

import { useState, useId } from "react";
import { useQueries } from "@tanstack/react-query";
import { AppWindow, HardDrive } from "lucide-react";
import { command } from "../../api/commands";
import { Usage } from "../../api/http";

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
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));

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
      open
      fullWidth
      maxWidth="sm"
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown") onClose();
      }}
    >
      <DialogTitle id={titleId}>
        {kind === "client" ? "More clients" : "More storage"}
      </DialogTitle>
      <DialogContent dividers>
        <Stack spacing={2}>
          <TextField
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
            <Typography variant="body2" color="text.secondary">
              Assignments on this page
            </Typography>
          )}
          {[...groups]
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([label, members]) => (
              <section key={label}>
                <Typography
                  component="h3"
                  variant="subtitle2"
                  sx={{ overflowWrap: "anywhere" }}
                >
                  {label} ({members.length})
                </Typography>
                <List disablePadding>
                  {members.map((id) => {
                    const summary = sumClients([id], totals);
                    const row = storages.find((row) => row.storage_id === id);
                    return (
                      <ListItem key={id} disablePadding divider>
                        <ListItemButton
                          onClick={() => onSelect(id)}
                          aria-label={`Show ${kind} ${id} on map`}
                        >
                          <ListItemIcon>
                            {kind === "client" ? (
                              <AppWindow size={16} />
                            ) : (
                              <HardDrive size={16} />
                            )}
                          </ListItemIcon>
                          <ListItemText
                            primary={id}
                            sx={{ overflowWrap: "anywhere" }}
                            secondary={
                              kind === "client"
                                ? summary
                                  ? `${summary.files.toLocaleString("en-US")} files · ${bytes(summary.bytes)}`
                                  : "Usage unavailable"
                                : row
                                  ? `${row.active_files.toLocaleString("en-US")} files · ${bytes(row.active_bytes)} / ${bytes(row.capacity_bytes)}`
                                  : "Usage unavailable"
                            }
                          />
                        </ListItemButton>
                      </ListItem>
                    );
                  })}
                </List>
              </section>
            ))}
          {!ids.length && (
            <Typography color="text.secondary">
              No matching {kind === "client" ? "clients" : "storage"}.
            </Typography>
          )}
        </Stack>
      </DialogContent>
      <TablePagination
        component="div"
        count={ids.length}
        page={current - 1}
        rowsPerPage={20}
        rowsPerPageOptions={[20]}
        onPageChange={(_event, next) => setPage(next + 1)}
      />
      <DialogActions>
        <Button onClick={onClose}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}
