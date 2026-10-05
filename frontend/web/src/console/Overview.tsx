import { useRef, useState } from "react";
import {
  Box,
  Button,
  Chip,
  Divider,
  Grid,
  Link,
  List,
  ListItemButton,
  ListItemText,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import { useQueries, useQuery } from "@tanstack/react-query";
import { command } from "../api/commands";
import { request, type Usage } from "../api/http";
import { type Client, type ClientUsage } from "../features/clients/model";
import {
  foldConnections,
  clientTotals,
} from "../features/overview/connectionsModel";
import { accountedBytes } from "../features/storages/capacity";
import { bytes } from "../design/format";
import { Capacity, Page, QueryState, Refresh } from "./ui";
import { ConnectionLines } from "./ConnectionLines";

export function Overview() {
  const connections = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<string | undefined>();
  const query = useQuery({
    queryKey: ["overview"],
    queryFn: async ({ signal }) => {
      const usage = await command<Usage[]>("usage.storages", {}, signal);
      const clients = await command<string[]>("client.list", {}, signal);
      const ready = await request<{ status: string }>("/readyz", {
        signal,
      }).then(
        (row) => row.status === "ready",
        () => false,
      );
      return { usage, clients, ready };
    },
  });
  const clientUsage = useQuery({
    queryKey: ["resource-usage", "client"],
    queryFn: ({ signal }) =>
      command<ClientUsage[]>("usage.clients", {}, signal),
  });
  const data = query.data;
  const totals =
    clientUsage.data && !clientUsage.isError
      ? clientTotals(clientUsage.data)
      : undefined;
  const clients = foldConnections(data?.clients ?? [], (id) => id);
  const assignments = useQueries({
    queries: clients.visible.map((id) => ({
      queryKey: ["clients", id],
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        command<Client>("client.show", { id }, signal),
    })),
  });
  const assignment = assignments.find(
    (row) => !row.isError && row.data?.id === selected,
  )?.data?.storage_id;
  const storages = foldConnections(
    data?.usage ?? [],
    (row) => row.storage_id,
    assignment,
  );
  const sum = (key: "active_files" | "active_bytes" | "capacity_bytes") =>
    data?.usage.reduce((sum, row) => sum + row[key], 0) ?? 0;
  return (
    <Page
      title="Overview"
      actions={
        <>
          <Button href="#usage">Usage history</Button>
          <Refresh
            disabled={query.isFetching}
            onClick={() => {
              void query.refetch();
              void clientUsage.refetch();
              for (const row of assignments) void row.refetch();
            }}
          />
        </>
      }
    >
      <QueryState
        pending={query.isPending}
        error={query.error}
        retry={() => void query.refetch()}
      />
      {data && !query.isError && (
        <>
          <Grid container spacing={2} aria-label="Usage summary">
            {[
              ["Clients", data.clients.length],
              ["Storage", data.usage.length],
              ["Stored files", sum("active_files").toLocaleString("en-US")],
              ["Stored data", bytes(sum("active_bytes"))],
            ].map(([label, value]) => (
              <Grid key={label} size={{ xs: 6, md: 3 }}>
                <Paper
                  variant="outlined"
                  component="section"
                  aria-label={String(label)}
                  sx={{ p: 2 }}
                >
                  <Typography variant="body2" color="text.secondary">
                    {label}
                  </Typography>
                  <Typography
                    variant="h5"
                    component="p"
                    sx={{ mt: 1, overflowWrap: "anywhere" }}
                  >
                    {value}
                  </Typography>
                </Paper>
              </Grid>
            ))}
          </Grid>
          <Stack component="section" aria-label="Connections" spacing={2}>
            <Stack
              direction="row"
              sx={{ justifyContent: "space-between", alignItems: "center" }}
            >
              <Typography component="h2" variant="h6">
                Connections
              </Typography>
              <Chip size="small" variant="outlined" label="Configured routes" />
            </Stack>
            <Box
              ref={connections}
              sx={{
                display: "grid",
                gridTemplateColumns: {
                  xs: "1fr",
                  md: "minmax(0, 1fr) 160px minmax(0, 1fr)",
                },
                gap: { xs: 3, md: 8 },
                position: "relative",
                alignItems: "center",
                minHeight: 280,
              }}
            >
              <ConnectionLines
                root={connections}
                revision={JSON.stringify([
                  clients.visible,
                  storages.visible.map((row) => row.storage_id),
                  selected,
                  assignment,
                ])}
              />
              <Stack spacing={1} sx={{ zIndex: 1 }}>
                <Typography variant="subtitle2">
                  Clients · {data.clients.length}
                </Typography>
                <List disablePadding>
                  {clients.visible.map((id) => (
                    <ListItemButton
                      component="a"
                      data-side="client"
                      data-selected={selected === id}
                      selected={selected === id}
                      href={`#clients/${encodeURIComponent(id)}`}
                      aria-label={`Open client ${id}`}
                      onMouseEnter={() => setSelected(id)}
                      onFocus={() => setSelected(id)}
                      key={id}
                      sx={{
                        bgcolor: "background.paper",
                        borderBottom: 1,
                        borderColor: "divider",
                      }}
                    >
                      <ListItemText
                        primary={id}
                        secondary={
                          totals
                            ? `${totals.get(id)?.files.toLocaleString("en-US") ?? "0"} files · ${bytes(totals.get(id)?.bytes ?? 0)}`
                            : "Usage unavailable"
                        }
                        sx={{ overflowWrap: "anywhere" }}
                      />
                    </ListItemButton>
                  ))}
                </List>
                <Link href="#clients" variant="body2">
                  {clients.hidden.length
                    ? `View all clients (+${clients.hidden.length})`
                    : "View all clients"}
                </Link>
              </Stack>
              <Stack
                data-hub
                spacing={1.5}
                sx={{
                  zIndex: 1,
                  alignItems: "center",
                  bgcolor: "background.default",
                  py: 3,
                }}
              >
                <Box
                  component="img"
                  src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
                  alt="Grove Storage"
                  sx={{ width: 64, height: 64 }}
                />
                <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
                  Grove Storage
                </Typography>
                <Chip
                  size="small"
                  color={data.ready ? "success" : "warning"}
                  label={data.ready ? "API ready" : "API not ready"}
                />
                <Typography variant="caption" color="text.secondary">
                  S3 gateway
                </Typography>
              </Stack>
              <Stack spacing={1} sx={{ zIndex: 1 }}>
                <Typography variant="subtitle2">
                  Storage · {data.usage.length}
                </Typography>
                {storages.visible.map((row) => (
                  <Box
                    key={row.storage_id}
                    data-side="storage"
                    data-selected={assignment === row.storage_id}
                    sx={{
                      bgcolor:
                        assignment === row.storage_id
                          ? "action.selected"
                          : "background.paper",
                      p: 1.5,
                    }}
                  >
                    <Link
                      href={`#storages/${encodeURIComponent(row.storage_id)}`}
                      aria-label={`Open storage ${row.storage_id}`}
                    >
                      {row.storage_id}
                    </Link>
                    <Capacity
                      used={accountedBytes(row)}
                      capacity={row.capacity_bytes}
                    />
                  </Box>
                ))}
                <Link href="#storages" variant="body2">
                  {storages.hidden.length
                    ? `View all storage (+${storages.hidden.length})`
                    : "View all storage"}
                </Link>
              </Stack>
            </Box>
            {!data.clients.length && (
              <Typography variant="body2" color="text.secondary">
                No clients registered.
              </Typography>
            )}
            {!data.usage.length && (
              <Typography variant="body2" color="text.secondary">
                No storage registered.
              </Typography>
            )}
            <Divider />
            {selected && (
              <Typography
                role="status"
                aria-label="Selected connection"
                variant="body2"
              >
                {assignment
                  ? `${selected} → ${assignment}`
                  : assignments[clients.visible.indexOf(selected)]?.isError
                    ? "Storage assignment unavailable"
                    : "Loading storage assignment..."}
              </Typography>
            )}
            <Typography variant="caption" color="text.secondary">
              Configured connections · Storage connectivity not checked
            </Typography>
          </Stack>
          <Stack spacing={2}>
            <Typography component="h2" variant="h6">
              Total allocation
            </Typography>
            <Capacity
              used={data.usage.reduce(
                (sum, row) => sum + accountedBytes(row),
                0,
              )}
              capacity={sum("capacity_bytes")}
            />
            <Typography variant="body2" color="text.secondary">
              Grove-managed usage and configured capacity, not provider free
              space.
            </Typography>
          </Stack>
          <TableContainer>
            <Table size="small" aria-label="Storage usage">
              <TableHead>
                <TableRow>
                  <TableCell>Storage</TableCell>
                  <TableCell align="right">Stored files</TableCell>
                  <TableCell align="right">Stored data</TableCell>
                  <TableCell align="right">Upload reservations</TableCell>
                  <TableCell align="right">Pending cleanup</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {data.usage.slice(0, 5).map((row) => (
                  <TableRow key={row.storage_id}>
                    <TableCell>
                      <Link
                        href={`#storages/${encodeURIComponent(row.storage_id)}`}
                      >
                        {row.storage_id}
                      </Link>
                    </TableCell>
                    <TableCell align="right">
                      {row.active_files.toLocaleString("en-US")}
                    </TableCell>
                    <TableCell align="right">
                      {bytes(row.active_bytes)}
                    </TableCell>
                    <TableCell align="right">
                      {bytes(row.reserved_bytes)}
                    </TableCell>
                    <TableCell align="right">
                      {bytes(row.purge_pending_bytes)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </>
      )}
    </Page>
  );
}
