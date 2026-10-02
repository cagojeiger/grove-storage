import { Divider, Grid, Link, Stack, Typography } from "@mui/material";
import { storageLink } from "../../app/navigation";
import { Details } from "../../app/Details";
import { bytes } from "../../design/format";
import { ResourceMetadata } from "../metadata/ResourceMetadata";
import type { Client } from "./model";

export function ClientDetail({
  client,
  usage,
  canWrite,
}: {
  client: Client;
  usage?: { files: number; bytes: number };
  canWrite: boolean;
}) {
  return (
    <Stack
      component="section"
      aria-label="Client details"
      spacing={3}
      divider={<Divider />}
    >
      <Stack spacing={2}>
        <Typography component="h2" variant="h6">
          Settings
        </Typography>
        <Details
          label="Client properties"
          items={[
            ["Client ID", client.id],
            ["S3 bucket", client.id],
            [
              "Storage",
              <Link href={storageLink(client.storage_id)}>
                {client.storage_id}
              </Link>,
            ],
          ]}
        />
      </Stack>
      <ResourceMetadata
        key={client.id}
        resource="client"
        id={client.id}
        canWrite={canWrite}
      />
      <Grid container spacing={3}>
        {[
          [
            "Stored files",
            usage ? usage.files.toLocaleString("en-US") : "Unavailable",
          ],
          ["Stored data", usage ? bytes(usage.bytes) : "Unavailable"],
        ].map(([label, value]) => (
          <Grid key={label} size={{ xs: 12, sm: 6 }}>
            <Typography variant="body2" color="text.secondary">
              {label}
            </Typography>
            <Typography component="p" variant="h5">
              {value}
            </Typography>
          </Grid>
        ))}
      </Grid>
    </Stack>
  );
}
