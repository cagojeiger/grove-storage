import { Divider, Grid, Stack, Typography } from "@mui/material";
import type { ReactNode } from "react";
import type { Storage } from "./model";
import type { Usage } from "../../api/http";
import { bytes } from "../../design/format";
import { ResourceMetadata } from "../metadata/ResourceMetadata";
import { CapacityUsage } from "./CapacityUsage";
import { accountedBytes } from "./capacity";
import { DetailSections } from "../../app/DetailSections";
import { Details } from "../../app/Details";

export function StorageDetail({
  storage,
  usage,
  canWrite,
  connection,
}: {
  storage: Storage;
  usage?: Usage;
  canWrite: boolean;
  connection: ReactNode;
}) {
  const fields: [string, ReactNode][] = [
    ["Type", storage.kind === "fs" ? "Filesystem" : "S3"],
    [
      "Configured capacity",
      `${bytes(storage.capacity_bytes)} (${storage.capacity_bytes.toLocaleString("en-US")} bytes)`,
    ],
    ...(storage.kind === "fs"
      ? [["Root path", storage.root_path] as [string, ReactNode]]
      : ([
          ["Endpoint", storage.endpoint],
          ["Public endpoint", storage.public_endpoint || "-"],
          ["Region", storage.region],
          ["Bucket", storage.bucket],
          ["Access key", storage.access_key],
          ["Path-style", storage.force_path_style ? "Enabled" : "Disabled"],
          ["Relay", storage.force_relay ? "Enabled" : "Disabled"],
        ] as [string, ReactNode][])),
  ];
  return (
    <DetailSections
      label="Storage sections"
      sections={[
        {
          value: "overview",
          label: "Overview",
          content: (
            <Stack spacing={3} divider={<Divider />}>
              <ResourceMetadata
                key={storage.id}
                resource="storage"
                id={storage.id}
                canWrite={canWrite}
              />
              <Stack component="section" aria-label="Storage usage" spacing={2}>
                <Typography component="h2" variant="h6">
                  Usage
                </Typography>
                <CapacityUsage
                  used={usage ? accountedBytes(usage) : null}
                  capacity={storage.capacity_bytes}
                />
                {usage ? (
                  <Grid container spacing={3}>
                    {[
                      {
                        label: "Stored data",
                        bytes: usage.active_bytes,
                        files: usage.active_files,
                      },
                      {
                        label: "Upload reservations",
                        bytes: usage.reserved_bytes,
                        files: usage.reserved_files,
                      },
                      {
                        label: "Pending cleanup",
                        bytes: usage.purge_pending_bytes,
                        files: usage.purge_pending_files,
                      },
                      {
                        label: "Remaining capacity",
                        bytes: usage.remaining_bytes,
                      },
                    ].map((counter) => (
                      <Grid key={counter.label} size={{ xs: 12, sm: 6 }}>
                        <Typography color="text.secondary" variant="body2">
                          {counter.label}
                        </Typography>
                        <Typography variant="h5" component="p">
                          {bytes(counter.bytes)}
                        </Typography>
                        {counter.files !== undefined && (
                          <Typography variant="body2" color="text.secondary">
                            Files: {counter.files.toLocaleString("en-US")}
                          </Typography>
                        )}
                      </Grid>
                    ))}
                  </Grid>
                ) : (
                  <Typography color="text.secondary">
                    Usage is unavailable.
                  </Typography>
                )}
              </Stack>
              {connection}
            </Stack>
          ),
        },
        {
          value: "configuration",
          label: "Configuration",
          content: (
            <Stack
              component="section"
              aria-label="Storage settings"
              spacing={2}
            >
              <Typography component="h2" variant="h6">
                Connection settings
              </Typography>
              <Details label="Storage properties" items={fields} />
            </Stack>
          ),
        },
      ]}
    />
  );
}
