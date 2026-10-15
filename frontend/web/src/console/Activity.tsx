import { useMemo, useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Alert,
  Box,
  Button,
  Chip,
  Collapse,
  Link,
  Stack,
  Tab,
  Tabs,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";
import { DataGrid, gridClasses, type GridColDef, type GridColumnVisibilityModel } from "@mui/x-data-grid";
import { useQuery } from "@tanstack/react-query";
import { field, identityPage } from "../api/identity";
import { cspNonce } from "../app/csp";
import { activityFilters, uuidPattern } from "../features/activity/filters";
import {
  actor,
  eventName,
  eventResult,
  invocationStatus,
  recordingStatus,
  sourceName,
  stageName,
  targetName,
  validator,
  type Event,
  type Stream,
} from "../features/activity/model";
import { duration, time } from "../design/format";
import { Filter } from "lucide-react";
import { Page, QueryState, Refresh } from "./ui";
import { ActivityDetails } from "./ActivityDetails";

export function Activity({ route, admin }: { route: string; admin: boolean }) {
  const mobile = useMediaQuery(useTheme().breakpoints.down("sm"));
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
  const [columnVisibility, setColumnVisibility] = useState<Record<"mobile" | "desktop", GridColumnVisibilityModel>>({
    desktop: {},
    mobile: { time: false, actor: false, source: false, duration: false, target: false, stage: false, recording: false },
  });
  const layout = mobile ? "mobile" : "desktop";
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
  const columns = useMemo<GridColDef<Event>[]>(() => [
    {
      field: "time",
      headerName: "Time",
      width: 190,
      valueGetter: (_, row) => time(row.context.created_at),
    },
    {
      field: "event",
      headerName: stream === "invocations" ? "Operation" : stream === "audit" ? "Action" : "Event",
      flex: 1,
      minWidth: mobile ? 140 : 190,
      renderCell: ({ row, tabIndex }) => (
        <Stack spacing={0.25} sx={{ minWidth: 0, height: "100%", justifyContent: "center" }}>
          <Tooltip title={eventName(row)}>
            <Link component="button" variant="body2" tabIndex={tabIndex} noWrap underline="hover" sx={{ maxWidth: "100%" }}
              onClick={(event) => { event.stopPropagation(); setSelected(row); }}>
              {eventName(row)}
            </Link>
          </Tooltip>
          {"outcome" in row && <Typography variant="caption" color="text.secondary" noWrap>{stageName(row.stage)}</Typography>}
          {"event_type" in row && <Tooltip title={row.operation ?? "Operation not recorded"}>
            <Typography variant="caption" color="text.secondary" noWrap>{row.operation ?? "Operation not recorded"}</Typography>
          </Tooltip>}
          {mobile && <Typography variant="caption" color="text.secondary" noWrap>{time(row.context.created_at)}</Typography>}
        </Stack>
      ),
    },
    {
      field: "result",
      headerName:
        stream === "audit"
          ? "Resource"
          : stream === "security"
            ? "Reason"
            : "Outcome",
      flex: stream === "invocations" ? undefined : 1,
      width: stream === "invocations" ? 125 : undefined,
      minWidth: mobile ? 110 : stream === "invocations" ? 125 : 180,
      valueGetter: (_, row) => eventResult(row),
      renderCell: stream === "invocations" ? ({ row }) => {
        if (!("outcome" in row)) return null;
        const outcome = invocationStatus(row.outcome);
        return <Chip label={outcome.label} color={outcome.color} variant="outlined" size="small" />;
      } : stream === "security" && mobile ? ({ row }) => {
        if (!("event_type" in row)) return null;
        const recording = recordingStatus(row.is_sampled);
        return <Stack spacing={0.5} sx={{ height: "100%", justifyContent: "center", minWidth: 0 }}>
          <Typography variant="body2" noWrap>{row.reason_code}</Typography>
          <Tooltip title={recording.description}>
            <Chip label={recording.label} color={recording.color} variant="outlined" size="small" sx={{ maxWidth: "100%" }} />
          </Tooltip>
        </Stack>;
      } : undefined,
    },
    ...(stream === "invocations" ? [{
      field: "target", headerName: "Target", flex: 1, minWidth: 180,
      valueGetter: (_: unknown, row: Event) => "outcome" in row ? targetName(row) : "",
    }] : []),
    ...(stream === "security" ? [{
      field: "stage", headerName: "Stage", width: 140,
      valueGetter: (_: unknown, row: Event) => "event_type" in row ? stageName(row.stage) : "",
    }, {
      field: "recording", headerName: "Recording", width: 150,
      valueGetter: (_: unknown, row: Event) => "event_type" in row ? recordingStatus(row.is_sampled).label : "",
      renderCell: ({ row }: { row: Event }) => {
        if (!("event_type" in row)) return null;
        const recording = recordingStatus(row.is_sampled);
        return <Tooltip title={recording.description}>
          <Chip label={recording.label} color={recording.color} variant="outlined" size="small" />
        </Tooltip>;
      },
    }] : []),
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
      valueGetter: (_, row) => sourceName(row.context.surface),
    },
    ...(stream === "invocations" ? [{
      field: "duration", headerName: "Duration", width: 110,
      valueGetter: (_: unknown, row: Event) => "duration_ms" in row ? duration(row.duration_ms) : "",
    }] : []),
  ], [mobile, stream]);
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
              startIcon={<Filter size={18} />}
              aria-expanded={filters}
              onClick={() => setFilters(!filters)}
            >
              Filters{params.size ? ` (${params.size})` : ""}
            </Button>
            {params.size > 0 && (
              <Button href={`#${path}`}>Clear filters</Button>
            )}
          </Stack>
          <Collapse in={filters} unmountOnExit>
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
                <DataGrid<Event>
                  nonce={cspNonce}
                  aria-label="Activity"
                  rows={rows}
                  getRowId={(row) => row.context.id}
                  columns={columns}
                  density={mobile ? "standard" : "compact"}
                  rowHeight={stream === "audit" ? undefined : mobile ? 80 : 64}
                  columnVisibilityModel={columnVisibility[layout]}
                  onColumnVisibilityModelChange={model => setColumnVisibility(current => ({ ...current, [layout]: model }))}
                  // Each cursor page has at most 50 rows; mounted cells support native focus scrolling.
                  disableVirtualization
                  // Hidden header menu buttons must not create a horizontal scroll range.
                  sx={{ [`& .${gridClasses["columnHeader--last"]}`]: { overflow: "hidden" } }}
                  slotProps={{ cell: { onFocus: event => event.currentTarget.scrollIntoView({ block: "nearest", inline: "nearest" }) } }}
                  loading={query.isPending}
                  disableColumnFilter
                  disableColumnSorting
                  disableRowSelectionOnClick
                  onRowClick={({ id }) => setSelected(rows.find((row) => row.context.id === id) ?? null)}
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
        <ActivityDetails key={selected.context.id} event={selected} onClose={() => setSelected(null)} />
      )}
    </Page>
  );
}
