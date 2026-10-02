import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  CircularProgress,
  Container,
  Grid,
  IconButton,
  Link,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw, History } from "lucide-react";
import { message, request, Usage } from "../../api/http";
import { command } from "../../api/commands";
import { bytes } from "../../design/format";
import { Connections } from "./Connections";
import { storageLink } from "../../app/navigation";
import { CapacityUsage } from "../storages/CapacityUsage";

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
      | "reserved_bytes"
      | "purge_pending_bytes",
  ) => data?.usage.reduce((total, row) => total + row[key], 0) ?? 0;
  return (
    <Container component="main" maxWidth="xl" sx={{ py: 3 }}>
      <Stack spacing={3}>
        <Stack
          direction={{ xs: "column", sm: "row" }}
          spacing={2}
          sx={{ alignItems: { sm: "center" }, justifyContent: "space-between" }}
        >
          <Typography component="h1" variant="h5">
            Overview
          </Typography>
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <Button href="#usage" startIcon={<History size={16} />}>
              Usage history
            </Button>
            <Tooltip title="Refresh">
              <span>
                <IconButton
                  aria-label="Refresh"
                  disabled={query.isFetching}
                  onClick={() => {
                    void query.refetch();
                    void cache.invalidateQueries({ queryKey: ["clients"] });
                  }}
                >
                  {query.isFetching ? (
                    <CircularProgress size={18} color="inherit" />
                  ) : (
                    <RefreshCw size={18} />
                  )}
                </IconButton>
              </span>
            </Tooltip>
          </Stack>
        </Stack>
        {query.isPending ? (
          <Typography role="status">Loading...</Typography>
        ) : query.isError ? (
          <Alert severity="error">{message(query.error)}</Alert>
        ) : (
          data && (
            <>
              <Grid
                container
                spacing={2}
                columns={{ xs: 2, sm: 6, lg: 5 }}
                aria-label="Usage summary"
              >
                {[
                  ["Clients", data.clients.length.toLocaleString("en-US")],
                  ["Storage", data.usage.length.toLocaleString("en-US")],
                  ["Stored files", sum("active_files").toLocaleString("en-US")],
                  ["Stored data", bytes(sum("active_bytes"))],
                  ["Configured capacity", bytes(sum("capacity_bytes"))],
                ].map(([label, value]) => (
                  <Grid key={label} size={{ xs: 1, sm: 2, lg: 1 }}>
                    <Card
                      component="section"
                      aria-label={label}
                      variant="outlined"
                      sx={{ height: "100%" }}
                    >
                      <CardContent>
                        <Typography
                          component="h2"
                          variant="body2"
                          color="text.secondary"
                        >
                          {label}
                        </Typography>
                        <Typography
                          component="p"
                          variant="h5"
                          sx={{ mt: 1, overflowWrap: "anywhere" }}
                        >
                          {value}
                        </Typography>
                      </CardContent>
                    </Card>
                  </Grid>
                ))}
              </Grid>
              <CapacityUsage used={sum("active_bytes") + sum("reserved_bytes") + sum("purge_pending_bytes")} capacity={sum("capacity_bytes")} />
              <Stack
                direction="row"
                spacing={3}
                useFlexGap
                sx={{ flexWrap: "wrap" }}
              >
                <Typography variant="body2" color="text.secondary">
                  Upload reservations {bytes(sum("reserved_bytes"))}
                </Typography>
                <Typography variant="body2" color="text.secondary">
                  Pending cleanup {bytes(sum("purge_pending_bytes"))}
                </Typography>
              </Stack>
              <Connections
                clients={data.clients}
                storages={data.usage}
                ready={data.ready}
              />
              <Box component="section">
                <Box
                  sx={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: 2,
                    mb: 2,
                  }}
                >
                  <Typography component="h2" variant="h6">
                    Storage usage
                  </Typography>
                  <Link href="#storages" variant="body2">
                    View all storage
                  </Link>
                </Box>
                <TableContainer>
                  <Table
                    size="small"
                    aria-label="Storage usage"
                    sx={{ minWidth: 640 }}
                  >
                    <TableHead>
                      <TableRow>
                        <TableCell>Storage</TableCell>
                        <TableCell align="right">Files</TableCell>
                        <TableCell align="right">Stored</TableCell>
                        <TableCell align="right">Upload reservations</TableCell>
                        <TableCell align="right">Pending cleanup</TableCell>
                        <TableCell align="right">Configured capacity</TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {[...data.usage]
                        .sort((a, b) =>
                          a.storage_id.localeCompare(b.storage_id),
                        )
                        .slice(0, 5)
                        .map((row) => (
                          <TableRow key={row.storage_id} hover>
                            <TableCell>
                              <Link href={storageLink(row.storage_id)}>
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
                            <TableCell align="right">
                              {bytes(row.capacity_bytes)}
                            </TableCell>
                          </TableRow>
                        ))}
                    </TableBody>
                  </Table>
                </TableContainer>
                <Typography
                  variant="body2"
                  color="text.secondary"
                  sx={{ mt: 1.5 }}
                >
                  Grove-managed usage and configured capacity, not
                  provider free space.
                </Typography>
              </Box>
            </>
          )
        )}
      </Stack>
    </Container>
  );
}
