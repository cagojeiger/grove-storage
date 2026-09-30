import {
  Alert,
  Box,
  IconButton,
  Button,
  Link,
  Stack,
  Tooltip,
  Typography,
  TextField,
} from "@mui/material";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, RefreshCw, Trash2 } from "lucide-react";
import { Page } from "../../app/Page";
import { command } from "../../api/commands";
import { message } from "../../api/http";
import { bytes } from "../../design/format";
import { storageLink } from "../../app/navigation";
import {
  Client,
  ClientUsage,
  clientLink,
  refreshClients,
  totals,
} from "./model";
import { ClientDialog } from "./ClientDialog";
import { ClientKeys } from "./ClientKeys";
import { ClientGrid } from "./ClientGrid";
import { ResourceMetadata } from "../metadata/ResourceMetadata";
import { paginate, useResourceList } from "../../app/resourceList";

export function Clients({
  route,
  canWrite,
}: {
  route: string;
  canWrite: boolean;
}) {
  const cache = useQueryClient();
  const [dialog, setDialog] = useState(false);
  const listing = useResourceList();
  let id = "";
  try {
    if (route.startsWith("clients/")) id = decodeURIComponent(route.slice(8));
  } catch {
    /* Invalid links fall back to the list. */
  }
  const list = useQuery({
    queryKey: ["clients", "list"],
    enabled: !id,
    queryFn: ({ signal }) => command<string[]>("client.list", {}, signal),
  });
  const detail = useQuery({
    queryKey: ["clients", "detail", id],
    enabled: Boolean(id),
    queryFn: ({ signal }) => command<Client>("client.show", { id }, signal),
  });
  const usage = useQuery({
    queryKey: ["clients", "usage"],
    queryFn: ({ signal }) =>
      command<ClientUsage[]>("usage.clients", {}, signal),
  });
  const current = id ? detail : list;
  const used = usage.isSuccess ? totals(usage.data, id) : undefined;
  const page = paginate(list.data ?? [], listing, (id) => id);
  return (
    <>
      <Page
        title={id || "Clients"}
        back={
          id ? { label: "Clients", href: listing.href("#clients") } : undefined
        }
        actions={
          <>
            <Tooltip title="Refresh clients">
              <span>
                <IconButton
                  type="button"
                  aria-label="Refresh clients"
                  disabled={current.isFetching || usage.isFetching}
                  onClick={() => void refreshClients(cache)}
                >
                  <RefreshCw size={18} />
                </IconButton>
              </span>
            </Tooltip>
            {canWrite &&
              (id ? (
                <Tooltip title="Delete client">
                  <span>
                    <IconButton
                      type="button"
                      color="error"
                      aria-label="Delete client"
                      disabled={!detail.data || current.isError}
                      onClick={() => setDialog(true)}
                    >
                      <Trash2 size={18} />
                    </IconButton>
                  </span>
                </Tooltip>
              ) : (
                <Button
                  type="button"
                  variant="contained"
                  startIcon={<Plus size={18} />}
                  onClick={() => setDialog(true)}
                >
                  Create client
                </Button>
              ))}
          </>
        }
      >
        {current.isPending ? (
          <Typography role="status" color="text.secondary" sx={{ py: 4 }}>
            Loading clients...
          </Typography>
        ) : current.isError ? (
          <Alert severity="error">{message(current.error)}</Alert>
        ) : id && detail.data ? (
          <>
            <Box
              component="dl"
              aria-label="Client details"
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
              {[
                { label: "Client ID", value: id },
                { label: "S3 bucket", value: id },
                {
                  label: "Storage",
                  value: (
                    <Link href={storageLink(detail.data.storage_id)}>
                      {detail.data.storage_id}
                    </Link>
                  ),
                },
                {
                  label: "Active files",
                  value:
                    usage.isError || !used
                      ? "Unavailable"
                      : used.files.toLocaleString("en-US"),
                },
                {
                  label: "Active data",
                  value:
                    usage.isError || !used ? "Unavailable" : bytes(used.bytes),
                },
              ].map(({ label, value }) => (
                <Stack key={label} spacing={0.5} sx={{ minWidth: 0 }}>
                  <Typography
                    component="dt"
                    variant="body2"
                    color="text.secondary"
                  >
                    {label}
                  </Typography>
                  <Typography
                    component="dd"
                    sx={{
                      m: 0,
                      overflowWrap: "anywhere",
                      fontVariantNumeric: "tabular-nums",
                    }}
                  >
                    {value}
                  </Typography>
                </Stack>
              ))}
              <ResourceMetadata
                key={`metadata:${id}`}
                resource="client"
                id={id}
                canWrite={canWrite}
              />
            </Box>
            {canWrite && <ClientKeys key={id} clientId={id} />}
          </>
        ) : (
          <>
            <TextField
              label="Search clients"
              type="search"
              value={listing.search}
              onChange={(event) =>
                listing.update({ search: event.target.value })
              }
              sx={{ maxWidth: 360 }}
            />
            <ClientGrid
              ids={page.rows}
              usage={usage.isSuccess ? usage.data : undefined}
              state={listing}
              page={page.page}
              total={page.total}
            />
          </>
        )}
        {usage.isError && (
          <Alert severity="error">
            Usage unavailable. {message(usage.error)}
          </Alert>
        )}
      </Page>
      {canWrite && dialog && (
        <ClientDialog
          id={id || undefined}
          onClose={() => setDialog(false)}
          onSaved={(target) => {
            setDialog(false);
            location.hash = listing.href(
              target ? clientLink(target) : "#clients",
            );
          }}
        />
      )}
    </>
  );
}
