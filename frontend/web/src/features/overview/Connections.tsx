import {
  Box,
  Button,
  Card,
  CardActionArea,
  CardContent,
  Chip,
  Stack,
  Typography,
} from "@mui/material";
import { useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowRight,
  HardDrive,
  AppWindow,
  Layers,
  CircleCheck,
  CircleX,
} from "lucide-react";
import { command } from "../../api/commands";
import { Usage } from "../../api/http";
import { bytes } from "../../design/format";
import { CapacityUsage } from "../storages/CapacityUsage";
import { accountedBytes } from "../storages/capacity";
import { storageLink } from "../../app/navigation";
import { Client, ClientUsage, clientLink } from "../clients/model";
import { clientTotals, foldConnections, sumClients } from "./connectionsModel";
import { ConnectionPaths } from "./ConnectionPaths";
import { ConnectionBrowser } from "./ConnectionBrowser";
import "./connections.css";

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
    <Stack spacing={2}>
      <Stack
        direction="row"
        spacing={2}
        sx={{ alignItems: "center", justifyContent: "space-between" }}
      >
        <Typography component="h2" variant="h6">
          Connections
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Configured routes
        </Typography>
      </Stack>
      <Box
        component="section"
        className="connections"
        aria-label="Storage connections"
        ref={graph}
      >
        <ConnectionPaths container={graph} revision={revision} />
        <Stack className="connection-column" spacing={2}>
          <Stack direction="row" sx={{ justifyContent: "space-between" }}>
            <Typography component="h3" variant="subtitle1">
              Clients
            </Typography>
            <Typography color="text.secondary">{clients.length}</Typography>
          </Stack>
          <Stack
            spacing={2}
            role="region"
            aria-label="Client connections"
            sx={{ flex: 1, justifyContent: "space-around" }}
          >
            {clientRows.visible.map((id) => {
              const summary = sumClients([id], totals);
              return (
                <Card
                  component="article"
                  variant="outlined"
                  key={id}
                  data-connection={`client:${id}`}
                  data-side="client"
                  data-selected={selectedClient === id}
                  sx={{
                    borderColor:
                      selectedClient === id ? "primary.main" : "divider",
                  }}
                >
                  <CardActionArea
                    component="a"
                    href={clientLink(id)}
                    aria-label={`Open client ${id}`}
                    onMouseEnter={() => chooseClient(id)}
                    onFocus={() => chooseClient(id)}
                  >
                    <CardContent>
                      <Stack
                        direction="row"
                        spacing={1}
                        sx={{ alignItems: "center" }}
                      >
                        <AppWindow size={18} />
                        <Typography
                          component="h4"
                          variant="subtitle2"
                          sx={{ overflowWrap: "anywhere" }}
                        >
                          {id}
                        </Typography>
                      </Stack>
                      <Stack
                        direction="row"
                        spacing={1}
                        useFlexGap
                        sx={{
                          mt: 1,
                          flexWrap: "wrap",
                          justifyContent: "space-between",
                        }}
                      >
                        <Typography variant="body2" color="text.secondary">
                          {summary
                            ? `${summary.files.toLocaleString("en-US")} files`
                            : "Usage unavailable"}
                        </Typography>
                        {summary && (
                          <Typography variant="body2">
                            {bytes(summary.bytes)}
                          </Typography>
                        )}
                      </Stack>
                    </CardContent>
                  </CardActionArea>
                </Card>
              );
            })}
            {clientRows.hidden.length > 0 && (
              <Card
                variant="outlined"
                data-connection="client:more"
                data-side="client"
              >
                <CardActionArea
                  aria-label={`Show ${clientRows.hidden.length} more clients`}
                  aria-haspopup="dialog"
                  onClick={() => setBrowser("client")}
                >
                  <CardContent>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{ alignItems: "center" }}
                    >
                      <Layers size={18} />
                      <Typography variant="subtitle2">
                        + {clientRows.hidden.length} clients
                      </Typography>
                    </Stack>
                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ mt: 1 }}
                    >
                      {hiddenClients
                        ? `${hiddenClients.files.toLocaleString("en-US")} files · ${bytes(hiddenClients.bytes)}`
                        : "Usage unavailable"}
                    </Typography>
                  </CardContent>
                </CardActionArea>
              </Card>
            )}
          </Stack>
          {!clients.length && (
            <Typography color="text.secondary">
              No clients registered.
            </Typography>
          )}
          <Button
            href="#clients"
            endIcon={<ArrowRight size={16} />}
            sx={{ alignSelf: "flex-start" }}
          >
            View all clients
          </Button>
        </Stack>
        <Box className="grove-hub">
          <Stack
            className="hub-label"
            spacing={1}
            sx={{ alignItems: "center", bgcolor: "background.default", py: 2 }}
          >
            <Box
              component="img"
              src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
              alt=""
              sx={{ width: 64, height: 64 }}
            />
            <Typography component="h3" variant="h6">
              Grove Storage
            </Typography>
            <Typography variant="body2" color="text.secondary">
              S3 gateway
            </Typography>
            <Chip
              size="small"
              color={ready ? "success" : "error"}
              variant="outlined"
              icon={ready ? <CircleCheck size={16} /> : <CircleX size={16} />}
              label={`API ${ready ? "ready" : "unavailable"}`}
            />
          </Stack>
        </Box>
        <Stack className="connection-column" spacing={2}>
          <Stack direction="row" sx={{ justifyContent: "space-between" }}>
            <Typography component="h3" variant="subtitle1">
              Storage
            </Typography>
            <Typography color="text.secondary">{storages.length}</Typography>
          </Stack>
          <Stack
            spacing={2}
            role="region"
            aria-label="Registered storage connections"
            sx={{ flex: 1, justifyContent: "space-around" }}
          >
            {storageRows.visible.map((row) => (
              <Card
                component="article"
                variant="outlined"
                key={row.storage_id}
                data-connection={`storage:${row.storage_id}`}
                data-side="storage"
                data-selected={row.storage_id === activeStorage}
                sx={{
                  borderColor:
                    row.storage_id === activeStorage
                      ? "primary.main"
                      : "divider",
                }}
              >
                <CardActionArea
                  component="a"
                  href={storageLink(row.storage_id)}
                  aria-label={`Open storage ${row.storage_id}`}
                  onMouseEnter={() => chooseStorage(row.storage_id)}
                  onFocus={() => chooseStorage(row.storage_id)}
                >
                  <CardContent>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{ alignItems: "center" }}
                    >
                      <HardDrive size={18} />
                      <Typography
                        component="h4"
                        variant="subtitle2"
                        sx={{ overflowWrap: "anywhere" }}
                      >
                        {row.storage_id}
                      </Typography>
                    </Stack>
                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ my: 1 }}
                    >
                      {row.kind.toUpperCase()} ·{" "}
                      {row.active_files.toLocaleString("en-US")} files
                    </Typography>
                    <CapacityUsage used={accountedBytes(row)} capacity={row.capacity_bytes} />
                  </CardContent>
                </CardActionArea>
              </Card>
            ))}
            {storageRows.hidden.length > 0 && (
              <Card
                variant="outlined"
                data-connection="storage:more"
                data-side="storage"
              >
                <CardActionArea
                  aria-label={`Show ${storageRows.hidden.length} more storage`}
                  aria-haspopup="dialog"
                  onClick={() => setBrowser("storage")}
                >
                  <CardContent>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{ alignItems: "center" }}
                    >
                      <Layers size={18} />
                      <Typography variant="subtitle2">
                        + {storageRows.hidden.length} storage
                      </Typography>
                    </Stack>
                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ mt: 1 }}
                    >
                      {hiddenStorage.files.toLocaleString("en-US")} files ·{" "}
                      {bytes(hiddenStorage.bytes)} active
                    </Typography>
                    <Typography variant="body2" color="text.secondary">
                      {bytes(hiddenStorage.capacity)} configured capacity
                    </Typography>
                  </CardContent>
                </CardActionArea>
              </Card>
            )}
          </Stack>
          {!storages.length && (
            <Typography color="text.secondary">
              No storage registered.
            </Typography>
          )}
        </Stack>
      </Box>
      <Typography
        variant="body2"
        color="text.secondary"
        role="status"
        aria-label="Selected connection"
      >
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
      </Typography>
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
    </Stack>
  );
}
