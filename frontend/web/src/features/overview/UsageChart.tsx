import { Stack, Tab, Tabs, Typography } from "@mui/material";
import { LineChart } from "@mui/x-charts/LineChart";
import { useMemo, useState } from "react";
import { bytes } from "../../design/format";
import { dailyUsage, Snapshot } from "./usageHistoryModel";

export function UsageChart({ rows }: { rows: Snapshot[] }) {
  const [metric, setMetric] = useState<"bytes" | "files">("bytes");
  const daily = useMemo(() => dailyUsage(rows), [rows]);
  const label = metric === "bytes" ? "Active data" : "Active files";
  const format = (value: number) =>
    metric === "bytes" ? bytes(value) : value.toLocaleString("en-US");
  return (
    <Stack component="section" aria-label="Recorded usage" spacing={1}>
      <Typography component="h2" variant="h6">
        Recorded usage
      </Typography>
      <Tabs
        value={metric}
        onChange={(_event, value: "bytes" | "files") => setMetric(value)}
        aria-label="Usage metric"
      >
        <Tab value="bytes" label="Active data" />
        <Tab value="files" label="Active files" />
      </Tabs>
      <LineChart
        height={280}
        title={`${label} by day`}
        desc="Sum of recorded daily snapshots in UTC. Missing days are gaps. Exact resource values are in the daily snapshots table."
        xAxis={[
          {
            scaleType: "point",
            data: daily.map((row) => row.day),
            valueFormatter: (day: string, context) =>
              context.location === "tick" ? day.slice(5) : day,
          },
        ]}
        yAxis={[{ min: 0, width: 80, valueFormatter: format }]}
        series={[
          {
            id: metric,
            label,
            data: daily.map((row) => row[metric]),
            curve: "linear",
            connectNulls: false,
            showMark: daily.length <= 31,
            valueFormatter: (value) =>
              value === null ? "Not recorded" : format(value),
          },
        ]}
        hideLegend
      />
    </Stack>
  );
}
