import type { ReactNode } from "react";
import { Grid, Stack } from "@mui/material";

export function Details({
  label,
  items,
}: {
  label: string;
  items: [string, ReactNode][];
}) {
  return (
    <Stack component="dl" aria-label={label} sx={{ m: 0 }}>
      {items.map(([name, value]) => (
        <Grid
          container
          key={name}
          spacing={1}
          sx={{ py: 1.5, borderBottom: 1, borderColor: "divider" }}
        >
          <Grid
            component="dt"
            size={{ xs: 12, sm: 4 }}
            sx={{ typography: "body2", color: "text.secondary" }}
          >
            {name}
          </Grid>
          <Grid
            component="dd"
            size={{ xs: 12, sm: 8 }}
            sx={{ typography: "body2", m: 0, overflowWrap: "anywhere" }}
          >
            {value ?? "-"}
          </Grid>
        </Grid>
      ))}
    </Stack>
  );
}
