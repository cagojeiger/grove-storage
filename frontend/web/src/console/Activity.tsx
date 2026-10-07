import { useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Alert,
  Box,
  Button,
  Collapse,
  DialogActions,
  DialogContent,
  DialogTitle,
  Drawer,
  Link,
  Stack,
  Tab,
  Tabs,
  Typography,
} from "@mui/material";
import { DataGrid, type GridColDef } from "@mui/x-data-grid";
import { useQuery } from "@tanstack/react-query";
import { field, identityPage } from "../api/identity";
import { cspNonce } from "../app/csp";
import { activityFilters, uuidPattern } from "../features/activity/filters";
import {
  actor,
  eventName,
  eventResult,
  validator,
  type Event,
  type Stream,
} from "../features/activity/model";
import { time } from "../design/format";
import { Page, Properties, QueryState, Refresh } from "./ui";

export function Activity({ route, admin }: { route: string; admin: boolean }) {
  const path = route.split("?")[0];
  const stream: Stream = path.endsWith("/security")
    ? "security"
    : path.endsWith("/invocations")
      ? "invocations"
      : "audit";
  const { params, valid } = activityFilters(route);
  const denied = stream === "security" && !admin;
  const [filters, setFilters] = useState(params.size > 0 || !valid);
  const [selected, setSelected] = useState<Event | null>(null);
  const [page, setPage] = useState(0);
  const [cursors, setCursors] = useState<(string | null)[]>([null]);
  const query = useQuery({
    queryKey: ["activity", stream, admin, params.toString(), cursors[page]],
    enabled: valid && !denied,
    queryFn: ({ signal }) =>
      identityPage(
        `/history/${stream}`,
        validator(stream),
        cursors[page],
        signal,
        params,
      ),
    gcTime: 0,
  });
  const suffix = params.size ? `?${params}` : "";
  const rows = query.data?.items ?? [];
  const columns: GridColDef<Event>[] = [
    {
      field: "time",
      headerName: "Time",
      width: 190,
      valueGetter: (_, row) => time(row.context.created_at),
    },
    {
      field: "event",
      headerName: "Event",
      flex: 1,
      minWidth: 190,
      renderCell: ({ row }) => (
        <Link component="button" onClick={() => setSelected(row)}>
          {eventName(row)}
        </Link>
      ),
    },
    {
      field: "result",
      headerName:
        stream === "audit"
          ? "Resource"
          : stream === "security"
            ? "Reason"
            : "Result",
      flex: 1,
      minWidth: 180,
      valueGetter: (_, row) => eventResult(row),
    },
    {
      field: "actor",
      headerName: "Actor",
      width: 180,
      valueGetter: (_, row) => actor(row.context),
    },
    {
      field: "source",
      headerName: "Source",
      width: 110,
      valueGetter: (_, row) => row.context.surface,
    },
  ];
  return (
    <Page
      fill
      title={admin ? "Activity" : "My activity"}
      actions={
        <Refresh
          label="Refresh activity"
          disabled={query.isFetching || denied || !valid}
          onClick={() => {
            setSelected(null);
            void query.refetch();
          }}
        />
      }
    >
      <Tabs
        value={denied ? false : stream}
        aria-label="Activity views"
        variant="scrollable"
        scrollButtons="auto"
      >
        <Tab
          label="Audit log"
          value="audit"
          component="a"
          href={`#activity${suffix}`}
        />
        <Tab
          label="Command history"
          value="invocations"
          component="a"
          href={`#activity/invocations${suffix}`}
        />
        {admin && (
          <Tab
            label="Security events"
            value="security"
            component="a"
            href={`#activity/security${suffix}`}
          />
        )}
      </Tabs>
      {!denied && (
        <>
          <Stack direction="row" spacing={2}>
            <Button
              aria-expanded={filters}
              onClick={() => setFilters(!filters)}
            >
              Filters{params.size ? ` (${params.size})` : ""}
            </Button>
            {params.size > 0 && (
              <Button href={`#${path}`}>Clear filters</Button>
            )}
          </Stack>
          <Collapse in={filters}>
            <Stack
              component="form"
              direction={{ xs: "column", sm: "row" }}
              spacing={2}
              onSubmit={(event) => {
                event.preventDefault();
                const data = new FormData(event.currentTarget);
                const next = new URLSearchParams();
                for (const key of ["account_id", "credential_id"]) {
                  const value = field(data, key).trim();
                  if (value) next.set(key, value);
                }
                location.hash = `${path}${next.size ? `?${next}` : ""}`;
              }}
            >
              <TextField
                fullWidth
                size="small"
                label="Actor account ID"
                name="account_id"
                defaultValue={params.get("account_id") ?? ""}
                slotProps={{ htmlInput: { pattern: uuidPattern } }}
              />
              <TextField
                fullWidth
                size="small"
                label="Used token ID"
                name="credential_id"
                defaultValue={params.get("credential_id") ?? ""}
                slotProps={{ htmlInput: { pattern: uuidPattern } }}
              />
              <Button type="submit">Apply</Button>
            </Stack>
          </Collapse>
        </>
      )}
      {denied ? (
        <Alert severity="error">Admin access required.</Alert>
      ) : !valid ? (
        <Alert severity="error">Enter a valid account or token ID.</Alert>
      ) : (
        <>
          <QueryState
            pending={query.isPending}
            error={query.error}
            retry={() => void query.refetch()}
          />
          {!query.isError && (
            <>
              <Box
                sx={{
                  display: "flex",
                  flexDirection: "column",
                  minHeight: 200,
                  height: 0,
                  flex: "1 1 0",
                  minWidth: 0,
                }}
              >
                <DataGrid
                  nonce={cspNonce}
                  aria-label="Activity"
                  rows={rows}
                  getRowId={(row) => row.context.id}
                  columns={columns}
                  loading={query.isPending}
                  disableColumnFilter
                  disableColumnSorting
                  disableRowSelectionOnClick
                  paginationMode="server"
                  paginationModel={{ page, pageSize: 50 }}
                  pageSizeOptions={[50]}
                  rowCount={-1}
                  paginationMeta={{
                    hasNextPage: query.data
                      ? Boolean(query.data.next_before)
                      : true,
                  }}
                  onPaginationModelChange={({ page: next }) => {
                    if (query.isFetching) return;
                    if (next === page + 1 && query.data?.next_before) {
                      setCursors([
                        ...cursors.slice(0, next),
                        query.data.next_before,
                      ]);
                      setPage(next);
                      setSelected(null);
                    } else if (next >= 0 && next < page) {
                      setPage(next);
                      setSelected(null);
                    }
                  }}
                  localeText={{
                    noRowsLabel: params.size
                      ? "No activity matches these filters."
                      : "No activity recorded yet.",
                  }}
                />
              </Box>
            </>
          )}
        </>
      )}
      {!denied && !query.isError && selected && (
        <Drawer
          anchor="right"
          open
          onClose={() => setSelected(null)}
          slotProps={{
            paper: {
              role: "dialog",
              "aria-labelledby": "event-title",
              sx: { width: { xs: "100%", sm: 480 } },
            },
          }}
        >
          <DialogTitle id="event-title">Event details</DialogTitle>
          <DialogContent dividers>
            <Stack spacing={3}>
              <Properties
                values={{
                  "Event ID": selected.context.id,
                  Event: eventName(selected),
                  Result: eventResult(selected),
                  Time: time(selected.context.created_at),
                  Actor: actor(selected.context),
                  Source: selected.context.surface,
                  "Request ID": selected.context.request_id,
                  "Token ID": selected.context.credential_id,
                  "Session ID": selected.context.session_id,
                  ...("duration_ms" in selected
                    ? {
                        "Duration (ms)": selected.duration_ms,
                        "Error code": selected.error_code,
                      }
                    : {}),
                }}
              />
              {"metadata" in selected && (
                <>
                  <Typography variant="subtitle2">Metadata</Typography>
                  <Typography
                    component="pre"
                    variant="body2"
                    sx={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}
                  >
                    {JSON.stringify(selected.metadata, null, 2)}
                  </Typography>
                </>
              )}
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setSelected(null)}>Close</Button>
          </DialogActions>
        </Drawer>
      )}
    </Page>
  );
}
