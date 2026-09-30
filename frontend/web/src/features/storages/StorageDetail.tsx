import {
  Card,
  CardContent,
  Divider,
  Grid,
  List,
  ListItem,
  ListItemText,
  Stack,
  Typography,
} from "@mui/material";
import type { Storage } from "./model";
import type { Usage } from "../../api/http";
import { bytes } from "../../design/format";
import { ResourceMetadata } from "../metadata/ResourceMetadata";

export function StorageDetail({
  storage,
  usage,
  canWrite,
}: {
  storage: Storage;
  usage?: Usage;
  canWrite: boolean;
}) {
  const fields = [
    ["Type", storage.kind === "fs" ? "Filesystem" : "S3"],
    [
      "Registered capacity",
      `${bytes(storage.capacity_bytes)} (${storage.capacity_bytes.toLocaleString("en-US")} bytes)`,
    ],
    ...(storage.kind === "fs"
      ? [["Root path", storage.root_path]]
      : [
          ["Endpoint", storage.endpoint],
          ["Public endpoint", storage.public_endpoint || "-"],
          ["Region", storage.region],
          ["Bucket", storage.bucket],
          ["Access key", storage.access_key],
          ["Path-style", storage.force_path_style ? "Enabled" : "Disabled"],
          ["Relay", storage.force_relay ? "Enabled" : "Disabled"],
        ]),
  ];
  return (
    <Stack spacing={3} divider={<Divider />}>
      <Stack component="section" aria-label="Storage settings" spacing={2}>
        <Typography component="h2" variant="h6">
          Settings
        </Typography>
        <Grid
          container
          component={List}
          disablePadding
          spacing={2}
          aria-label="Storage properties"
        >
          {fields.map(([label, value]) => (
            <Grid
              key={label}
              component={ListItem}
              disablePadding
              size={{ xs: 12, sm: 6 }}
            >
              <ListItemText
                primary={label}
                secondary={value || "-"}
                slotProps={{
                  primary: { variant: "body2", color: "text.secondary" },
                  secondary: {
                    variant: "body1",
                    color: "text.primary",
                    sx: { overflowWrap: "anywhere" },
                  },
                }}
              />
            </Grid>
          ))}
        </Grid>
        <ResourceMetadata
          key={storage.id}
          resource="storage"
          id={storage.id}
          canWrite={canWrite}
        />
      </Stack>
      <Stack component="section" aria-label="Storage usage" spacing={2}>
        <Typography component="h2" variant="h6">
          Usage
        </Typography>
        {usage ? (
          <Grid container spacing={2}>
            {[
              {
                label: "Active",
                bytes: usage.active_bytes,
                files: usage.active_files,
              },
              {
                label: "Reserved",
                bytes: usage.reserved_bytes,
                files: usage.reserved_files,
              },
              {
                label: "Pending deletion",
                bytes: usage.purge_pending_bytes,
                files: usage.purge_pending_files,
              },
              { label: "Remaining", bytes: usage.remaining_bytes },
            ].map((counter) => (
              <Grid key={counter.label} size={{ xs: 12, sm: 6, lg: 3 }}>
                <Card variant="outlined" sx={{ height: "100%" }}>
                  <CardContent>
                    <Typography color="text.secondary" variant="body2">
                      {counter.label}
                    </Typography>
                    <Typography variant="h5" component="p" sx={{ my: 1 }}>
                      {bytes(counter.bytes)}
                    </Typography>
                    {counter.files !== undefined && (
                      <Typography variant="body2" color="text.secondary">
                        Files: {counter.files.toLocaleString("en-US")}
                      </Typography>
                    )}
                  </CardContent>
                </Card>
              </Grid>
            ))}
          </Grid>
        ) : (
          <Typography color="text.secondary">Usage is unavailable.</Typography>
        )}
      </Stack>
    </Stack>
  );
}
