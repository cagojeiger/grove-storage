import { useState } from "react";
import { Alert, Box, Button, Stack, Tab, Tabs, TextField } from "@mui/material";
import { LineChart } from "@mui/x-charts/LineChart";
import { DataGrid } from "@mui/x-data-grid";
import { useQuery } from "@tanstack/react-query";
import { command } from "../api/commands";
import { bytes } from "../design/format";
import { cspNonce } from "../app/csp";
import {
  dailyUsage,
  type Snapshot,
} from "../features/overview/usageHistoryModel";
import { Page, QueryState, Refresh } from "./ui";

export function UsageHistory() {
  const [days, setDays] = useState(90);
  const [metric, setMetric] = useState<"bytes" | "files">("bytes");
  const query = useQuery({
    queryKey: ["usage-history", days],
    queryFn: ({ signal }) =>
      command<Snapshot[]>("usage.history", { days }, signal),
  });
  const daily = dailyUsage(query.data ?? []);
  const label = metric === "bytes" ? "Stored data" : "Stored files";
  return (
    <Page
      title="Usage history"
      parent={{ label: "Overview", href: "#" }}
      actions={
        <Refresh
          label="Refresh usage history"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        />
      }
    >
      <Stack
        component="form"
        direction="row"
        spacing={2}
        onSubmit={(event) => {
          event.preventDefault();
          const value = new FormData(event.currentTarget).get("days");
          const next = Number(value);
          if (Number.isInteger(next) && next >= 1 && next <= 3650) setDays(next);
        }}
      >
        <TextField
          label="Days"
          name="days"
          type="number"
          defaultValue={days}
          size="small"
          required
          slotProps={{ htmlInput: { min: 1, max: 3650 } }}
        />
        <Button type="submit">Apply</Button>
      </Stack>
      <QueryState
        pending={query.isPending}
        error={query.error}
        retry={() => void query.refetch()}
      />
      {!query.isPending && !query.isError && (
        <>
          <Tabs
            aria-label="Usage metric"
            value={metric}
            onChange={(_, value: "bytes" | "files") => setMetric(value)}
          >
            <Tab value="bytes" label="Stored data" />
            <Tab value="files" label="Stored files" />
          </Tabs>
          {daily.length ? (
            <>
              <Box
                role="img"
                aria-label={`${label} by day`}
                sx={{ minWidth: 0 }}
              >
                <LineChart
                  height={300}
                  yAxis={[
                    {
                      valueFormatter: (value: number) =>
                        metric === "bytes"
                          ? bytes(Number(value))
                          : Number(value).toLocaleString("en-US"),
                    },
                  ]}
                  xAxis={[
                    { data: daily.map((row) => row.day), scaleType: "point" },
                  ]}
                  series={[
                    {
                      label,
                      data: daily.map((row) => row[metric]),
                      connectNulls: false,
                      showMark: daily.length <= 31,
                      curve: "linear",
                      valueFormatter: (value) =>
                        value === null
                          ? "No snapshot"
                          : metric === "bytes"
                            ? bytes(value)
                            : value.toLocaleString("en-US"),
                    },
                  ]}
                />
              </Box>
              <Box
                component="section"
                aria-label="Daily snapshots"
                sx={{ height: 400, minWidth: 0 }}
              >
                <DataGrid
                  nonce={cspNonce}
                  aria-label="Daily snapshots"
                  rows={query.data ?? []}
                  getRowId={(row) =>
                    JSON.stringify([row.day, row.storage_id, row.client_id])
                  }
                  columns={[
                    { field: "day", headerName: "Date (UTC)", width: 140 },
                    {
                      field: "observed_at",
                      headerName: "Observed at",
                      width: 200,
                      valueFormatter: (value: string | null) =>
                        value ? new Date(value).toLocaleString() : "Unknown",
                    },
                    {
                      field: "storage_id",
                      headerName: "Storage",
                      flex: 1,
                      minWidth: 140,
                    },
                    {
                      field: "client_id",
                      headerName: "Client",
                      flex: 1,
                      minWidth: 140,
                    },
                    {
                      field: "active_files",
                      headerName: "Stored files",
                      width: 130,
                      valueFormatter: (value: number) =>
                        value.toLocaleString("en-US"),
                    },
                    {
                      field: "active_bytes",
                      headerName: "Stored data",
                      width: 140,
                      valueFormatter: (value: number) => bytes(value),
                    },
                  ]}
                  disableRowSelectionOnClick
                  pageSizeOptions={[20, 50, 100]}
                  initialState={{
                    sorting: { sortModel: [{ field: "day", sort: "desc" }] },
                    pagination: { paginationModel: { pageSize: 20 } },
                  }}
                />
              </Box>
            </>
          ) : (
            <Alert severity="info">No usage snapshots recorded.</Alert>
          )}
        </>
      )}
    </Page>
  );
}
