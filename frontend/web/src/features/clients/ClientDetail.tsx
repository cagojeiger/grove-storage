import {
  Card,
  CardContent,
  Grid,
  Link,
  List,
  ListItem,
  ListItemText,
  Stack,
  Typography,
} from "@mui/material";
import { storageLink } from "../../app/navigation";
import { bytes } from "../../design/format";
import { ResourceMetadata } from "../metadata/ResourceMetadata";
import type { Client } from "./model";
import type { ReactNode } from "react";

export function ClientDetail({
  client,
  usage,
  canWrite,
}: {
  client: Client;
  usage?: { files: number; bytes: number };
  canWrite: boolean;
}) {
  const fields: [string, ReactNode][] = [
    ["Client ID", client.id],
    ["S3 bucket", client.id],
    [
      "Storage",
      <Link href={storageLink(client.storage_id)}>{client.storage_id}</Link>,
    ],
  ];
  return (
    <Stack component="section" aria-label="Client details" spacing={3}>
      <Typography component="h2" variant="h6">
        Settings
      </Typography>
      <Grid
        container
        component={List}
        disablePadding
        spacing={2}
        aria-label="Client properties"
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
              secondary={value}
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
        key={client.id}
        resource="client"
        id={client.id}
        canWrite={canWrite}
      />
      <Grid container spacing={2}>
        {[
          [
            "Active files",
            usage ? usage.files.toLocaleString("en-US") : "Unavailable",
          ],
          ["Active data", usage ? bytes(usage.bytes) : "Unavailable"],
        ].map(([label, value]) => (
          <Grid key={label} size={{ xs: 12, sm: 6 }}>
            <Card variant="outlined">
              <CardContent>
                <Typography variant="body2" color="text.secondary">
                  {label}
                </Typography>
                <Typography component="p" variant="h5" sx={{ mt: 1 }}>
                  {value}
                </Typography>
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Stack>
  );
}
