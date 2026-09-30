import { Box, Divider, Stack, Typography } from "@mui/material";
import type { ReactNode } from "react";
import { Storage } from "./model";
import { Usage } from "../../api/http";
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
  const fields: [string, ReactNode][] =
    storage.kind === "fs"
      ? [["Root path", storage.root_path]]
      : [
          ["Endpoint", storage.endpoint],
          ["Public endpoint", storage.public_endpoint],
          ["Region", storage.region],
          ["Bucket", storage.bucket],
          ["Access key", storage.access_key],
          ["Path-style", storage.force_path_style ? "Enabled" : "Disabled"],
          ["Relay", storage.force_relay ? "Enabled" : "Disabled"],
        ];
  return (
    <Stack spacing={3} divider={<Divider />}>
      <Stack component="section" aria-label="Storage settings" spacing={2}>
        <Typography variant="h2">Settings</Typography>
        <Box
          component="dl"
          sx={{
            m: 0,
            display: "grid",
            gridTemplateColumns: {
              xs: "minmax(0, 1fr)",
              sm: "repeat(2, minmax(0, 1fr))",
            },
            gap: 2.5,
          }}
        >
          <Stack spacing={0.5}>
            <Typography component="dt" variant="body2" color="text.secondary">
              Type
            </Typography>
            <Typography component="dd" sx={{ m: 0 }}>
              {storage.kind === "fs" ? "Filesystem" : "S3"}
            </Typography>
          </Stack>
          <Stack spacing={0.5}>
            <Typography component="dt" variant="body2" color="text.secondary">
              Registered capacity
            </Typography>
            <Typography component="dd" sx={{ m: 0, overflowWrap: "anywhere" }}>
              {bytes(storage.capacity_bytes)}{" "}
              <Typography
                component="span"
                variant="body2"
                color="text.secondary"
              >
                ({storage.capacity_bytes.toLocaleString("en-US")} bytes)
              </Typography>
            </Typography>
          </Stack>
          {fields.map(([label, value]) => (
            <Stack key={label} spacing={0.5} sx={{ minWidth: 0 }}>
              <Typography component="dt" variant="body2" color="text.secondary">
                {label}
              </Typography>
              <Typography
                component="dd"
                sx={{ m: 0, overflowWrap: "anywhere" }}
              >
                {value || "-"}
              </Typography>
            </Stack>
          ))}
          <ResourceMetadata
            key={storage.id}
            resource="storage"
            id={storage.id}
            canWrite={canWrite}
          />
        </Box>
      </Stack>
      <Stack component="section" aria-label="Storage usage" spacing={2}>
        <Typography variant="h2">Usage</Typography>
        {usage ? (
          <Box
            component="dl"
            sx={{
              m: 0,
              display: "grid",
              gridTemplateColumns: {
                xs: "repeat(2, minmax(0, 1fr))",
                md: "repeat(4, minmax(0, 1fr))",
              },
              gap: 2.5,
            }}
          >
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
              <Stack key={counter.label} spacing={0.5} sx={{ minWidth: 0 }}>
                <Typography
                  component="dt"
                  variant="body2"
                  color="text.secondary"
                >
                  {counter.label}
                </Typography>
                <Typography
                  component="dd"
                  sx={{
                    m: 0,
                    fontVariantNumeric: "tabular-nums",
                    overflowWrap: "anywhere",
                  }}
                >
                  {bytes(counter.bytes)}
                </Typography>
                {counter.files !== undefined && (
                  <Typography
                    component="dd"
                    variant="body2"
                    color="text.secondary"
                    sx={{ m: 0 }}
                  >
                    Files: {counter.files.toLocaleString("en-US")}
                  </Typography>
                )}
              </Stack>
            ))}
          </Box>
        ) : (
          <Typography color="text.secondary">Usage is unavailable.</Typography>
        )}
      </Stack>
    </Stack>
  );
}
