import { LinearProgress, Stack, Typography } from "@mui/material";
import { bytes } from "../../design/format";

export function CapacityUsage({ used, capacity }: { used: number | null; capacity: number }) {
  const percent = used !== null && capacity > 0 ? used / capacity * 100 : null;
  const over = used !== null && used > capacity;
  return (
    <Stack spacing={0.5} sx={{ width: "100%", minWidth: 0 }}>
      <Stack direction="row" spacing={1} sx={{ justifyContent: "space-between" }}>
        <Typography variant="body2">{used === null ? "Unavailable" : `${bytes(used)} / ${bytes(capacity)}`}</Typography>
        <Typography variant="body2" color={over ? "error" : "text.secondary"}>
          {percent === null ? "—" : `${percent.toLocaleString("en-US", { maximumFractionDigits: 1 })}%`}
        </Typography>
      </Stack>
      {percent !== null && <LinearProgress variant="determinate" value={Math.min(percent, 100)} color={over ? "error" : "primary"} aria-label="Accounted capacity" aria-valuetext={`${percent.toLocaleString("en-US", { maximumFractionDigits: 1 })}% of configured capacity`} />}
      <Typography variant="caption" color={over ? "error" : "text.secondary"}>
        {used === null ? "Usage unavailable" : over ? `Over capacity by ${bytes(used - capacity)}` : `${bytes(capacity - used)} remaining`}
      </Typography>
    </Stack>
  );
}
