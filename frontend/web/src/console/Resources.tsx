import { useEffect, useMemo, useRef, useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Alert,
  Box,
  Button,
  Divider,
  Link,
  Stack,
  Typography,
} from "@mui/material";
import {
  DataGrid,
  type GridColDef,
  type GridSortModel,
} from "@mui/x-data-grid";
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, PlugZap } from "lucide-react";
import { command } from "../api/commands";
import { message, type Usage } from "../api/http";
import { cspNonce } from "../app/csp";
import { useResourceList, paginate } from "../app/resourceList";
import { Storage, mutationMessage } from "../features/storages/model";
import { accountedBytes } from "../features/storages/capacity";
import {
  type Client,
  type ClientUsage,
  clientMessage,
} from "../features/clients/model";
import { bytes } from "../design/format";
import { useAction } from "../features/access/useAction";
import {
  Capacity,
  Confirmation,
  FormDialog,
  Page,
  Properties,
  QueryState,
  Refresh,
  SecretDialog,
} from "./ui";
import { StorageForm, ClientForm } from "./ResourceForms";
import { Metadata } from "./Metadata";

export function Resources({
  kind,
  id,
  writable,
  editor = false,
}: {
  kind: "storage" | "client";
  id?: string;
  writable: boolean;
  editor?: boolean;
}) {
  const cache = useQueryClient();
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const listing = useResourceList();
  const sortModel = useMemo<GridSortModel>(
    () => [{ field: "id", sort: listing.sort === "desc" ? "desc" : "asc" }],
    [listing.sort],
  );
  const title = kind === "storage" ? "Storage" : "Clients";
  const base = kind === "storage" ? "storages" : "clients";
  const [editing, setEditing] = useState(editor);
  const [deleting, setDeleting] = useState(false);
  const query = useQuery<Storage | Client | Storage[] | string[]>({
    queryKey: [base, id ?? "list"],
    queryFn: ({ signal }) =>
      id
        ? command<Storage | Client>(`${kind}.show`, { id }, signal)
        : command<Storage[] | string[]>(`${kind}.list`, {}, signal),
  });
  const usage = useQuery<Usage[] | ClientUsage[]>({
    queryKey: ["resource-usage", kind],
    queryFn: ({ signal }) =>
      kind === "storage"
        ? command<Usage[]>("usage.storages", {}, signal)
        : command<ClientUsage[]>("usage.clients", {}, signal),
  });
  async function refresh(target?: string) {
    if (mounted.current) {
      setEditing(false);
      setDeleting(false);
    }
    await cache.invalidateQueries();
    if (mounted.current && target !== undefined)
      location.hash = listing.href(
        `#${base}${target ? `/${encodeURIComponent(target)}` : ""}`,
      );
  }
  const record =
    id && query.data && !Array.isArray(query.data) && !query.isError
      ? query.data
      : undefined;
  const rows =
    !id && Array.isArray(query.data)
      ? query.data.map((item) =>
          typeof item === "string" ? { id: item } : item,
        )
      : [];
  const page = paginate(rows, listing, (row) => row.id);
  const assignments = useQueries({
    queries:
      kind === "client" && !id
        ? page.rows.map((row) => ({
            queryKey: ["clients", row.id],
            queryFn: ({ signal }: { signal: AbortSignal }) =>
              command<Client>("client.show", { id: row.id }, signal),
          }))
        : [],
  });
  const columns: GridColDef[] = [
    {
      field: "id",
      headerName: kind === "storage" ? "Storage ID" : "Client ID",
      flex: 1,
      minWidth: 170,
      renderCell: (params) => (
        <Link
          href={listing.href(
            `#${base}/${encodeURIComponent(params.value as string)}`,
          )}
        >
          {params.value}
        </Link>
      ),
    },
    ...(kind === "storage"
      ? [
          { field: "endpoint", headerName: "Endpoint", flex: 1, minWidth: 180 },
          {
            field: "capacity",
            headerName: "Capacity",
            width: 220,
            sortable: false,
            renderCell: (params: { row: { id: string } }) => {
              const row = (usage.data as Usage[] | undefined)?.find(
                (row) => row.storage_id === params.row.id,
              );
              return row ? (
                <Box sx={{ width: "100%", py: 1 }}>
                  <Capacity
                    used={accountedBytes(row)}
                    capacity={row.capacity_bytes}
                  />
                </Box>
              ) : (
                "—"
              );
            },
          },
        ]
      : [
          {
            field: "storage",
            headerName: "Storage",
            width: 180,
            sortable: false,
            renderCell: (params: { row: { id: string } }) => {
              const query =
                assignments[
                  page.rows.findIndex((row) => row.id === params.row.id)
                ];
              const storage = !query?.isError && query?.data?.storage_id;
              return storage ? (
                <Link href={`#storages/${encodeURIComponent(storage)}`}>
                  {storage}
                </Link>
              ) : query?.isError ? (
                "Unavailable"
              ) : (
                "Loading..."
              );
            },
          },
          {
            field: "files",
            headerName: "Stored files",
            width: 150,
            sortable: false,
            valueGetter: (_: unknown, row: { id: string }) =>
              usage.isError || !usage.data
                ? null
                : (usage.data as ClientUsage[])
                    .filter((item) => item.client_id === row.id)
                    .reduce((sum, row) => sum + row.active_files, 0),
          },
          {
            field: "bytes",
            headerName: "Stored data",
            width: 150,
            sortable: false,
            valueGetter: (_: unknown, row: { id: string }) =>
              usage.isError || !usage.data
                ? "—"
                : bytes(
                    (usage.data as ClientUsage[])
                      .filter((item) => item.client_id === row.id)
                      .reduce((sum, row) => sum + row.active_bytes, 0),
                  ),
          },
        ]),
  ];
  return (
    <Page
      fill={!id}
      title={id ?? title}
      parent={id ? { label: title, href: listing.href(`#${base}`) } : undefined}
      actions={
        <>
          <Refresh disabled={query.isFetching} onClick={() => void refresh()} />
          {writable &&
            (!id ? (
              <Button
                variant="contained"
                startIcon={<Plus size={18} />}
                onClick={() => setEditing(true)}
              >
                {kind === "storage" ? "Add storage" : "Create client"}
              </Button>
            ) : (
              record && (
                <>
                  {kind === "storage" && (
                    <Button
                      variant="outlined"
                      disabled={(record as Storage).kind !== "s3"}
                      onClick={() => setEditing(true)}
                    >
                      Edit storage
                    </Button>
                  )}
                  <Button color="error" onClick={() => setDeleting(true)}>
                    Delete {kind}
                  </Button>
                </>
              )
            ))}
        </>
      }
    >
      <QueryState
        pending={query.isPending}
        error={query.error}
        retry={() => void query.refetch()}
      />
      {!id && !query.isError && (
        <>
          <TextField
            type="search"
            size="small"
            label={`Search ${kind === "storage" ? "storage" : "clients"}`}
            value={listing.search}
            onChange={(event) => listing.update({ search: event.target.value })}
            sx={{ maxWidth: 400 }}
          />
          <Box
            sx={{
              display: "flex",
              flexDirection: "column",
              minHeight: 200,
              height: 0,
              flex: "1 1 0",
              minWidth: 0,
            }}
          >
            <DataGrid
              nonce={cspNonce}
              aria-label={title}
              rows={page.rows}
              columns={columns}
              rowCount={page.total}
              loading={query.isPending}
              paginationMode="server"
              paginationModel={{ page: page.page - 1, pageSize: page.size }}
              onPaginationModelChange={(model) =>
                listing.update(
                  model.pageSize !== page.size
                    ? { size: model.pageSize }
                    : { page: model.page + 1 },
                )
              }
              pageSizeOptions={[20, 50, 100]}
              sortingMode="server"
              sortModel={sortModel}
              sortingOrder={["asc", "desc"]}
              onSortModelChange={(model) =>
                listing.update({
                  sort: model[0]?.sort === "desc" ? "desc" : "asc",
                })
              }
              disableRowSelectionOnClick
              disableColumnFilter
              localeText={{
                noRowsLabel: listing.search
                  ? `No matching ${kind === "storage" ? "storage" : "clients"}.`
                  : kind === "storage"
                    ? "No storage registered."
                    : "No clients registered.",
              }}
              rowHeight={64}
            />
          </Box>
        </>
      )}
      {usage.isError && (
        <Alert severity="warning">
          Usage unavailable. {message(usage.error)}
        </Alert>
      )}
      {record && (
        <Stack
          component="section"
          aria-label={
            kind === "storage" ? "Storage settings" : "Client details"
          }
          spacing={3}
        >
          <Properties
            values={
              kind === "storage"
                ? storageProperties(record as Storage)
                : {
                    "Client ID": record.id,
                    "S3 bucket": record.id,
                    Storage: (
                      <Link
                        href={`#storages/${encodeURIComponent((record as Client).storage_id)}`}
                      >
                        {(record as Client).storage_id}
                      </Link>
                    ),
                  }
            }
          />
          <Divider />
          <Metadata
            key={`${kind}:${id}`}
            kind={kind}
            id={id!}
            writable={writable}
          />
          <Divider />
          {kind === "storage" ? (
            <>
              <Stack component="section" aria-label="Usage" spacing={2}>
                <Typography variant="h6" component="h2">
                  Usage
                </Typography>
                {!usage.isError &&
                  (usage.data as Usage[] | undefined)
                    ?.filter((row) => row.storage_id === id)
                    .map((row) => (
                      <Stack key={row.storage_id} spacing={2}>
                        <Capacity
                          used={accountedBytes(row)}
                          capacity={row.capacity_bytes}
                        />
                        <Properties
                          values={{
                            "Stored files":
                              row.active_files.toLocaleString("en-US"),
                            "Stored data": bytes(row.active_bytes),
                            "Upload reservations": bytes(row.reserved_bytes),
                            "Pending cleanup": bytes(row.purge_pending_bytes),
                            "Remaining allocation": bytes(row.remaining_bytes),
                          }}
                        />
                        <Typography variant="caption" color="text.secondary">
                          Grove-managed usage and configured capacity, not
                          provider free space.
                        </Typography>
                      </Stack>
                    ))}
              </Stack>
              <Divider />
              <Connection
                id={id!}
                revision={query.dataUpdatedAt}
                refreshing={query.isFetching}
              />
            </>
          ) : (
            <>
              <Properties
                values={{
                  "Stored files":
                    !usage.isError && usage.data
                      ? (usage.data as ClientUsage[])
                          .filter((row) => row.client_id === id)
                          .reduce((sum, row) => sum + row.active_files, 0)
                          .toLocaleString("en-US")
                      : "Unavailable",
                  "Stored data":
                    !usage.isError && usage.data
                      ? bytes(
                          (usage.data as ClientUsage[])
                            .filter((row) => row.client_id === id)
                            .reduce((sum, row) => sum + row.active_bytes, 0),
                        )
                      : "Unavailable",
                }}
              />
              <Divider />
              {writable && <ServiceKeys id={id!} writable={writable} />}
            </>
          )}
        </Stack>
      )}
      {editing && (!id || record) &&
        (kind === "storage" ? (
          <StorageForm
            storage={record as Storage | undefined}
            onClose={() => void refresh(editor ? id ?? "" : undefined)}
            onSaved={refresh}
          />
        ) : (
          <ClientForm onClose={() => void refresh()} onSaved={refresh} />
        ))}
      {deleting && id && (
        <Confirmation
          title={`Delete ${kind}`}
          text="Referenced files and pending cleanup block deletion. This removes the registration and its associated keys."
          confirm={id}
          label={
            kind === "storage" ? "Storage ID to delete" : "Client ID to delete"
          }
          button="Confirm delete"
          onClose={() => void refresh()}
          errorMessage={
            kind === "storage"
              ? (error) => mutationMessage(error, "delete")
              : clientMessage
          }
          execute={async () => {
            await command(`${kind}.delete`, { id });
            await refresh("");
          }}
        />
      )}
    </Page>
  );
}
function storageProperties(storage: Storage) {
  if (storage.kind !== "s3")
    return {
      Type: storage.kind.toUpperCase(),
      "Root path": storage.root_path,
      "Configured capacity": bytes(storage.capacity_bytes),
    };
  return {
    Type: storage.kind.toUpperCase(),
    Endpoint: storage.endpoint,
    "Public endpoint": storage.public_endpoint,
    Region: storage.region,
    Bucket: storage.bucket,
    "Access key": storage.access_key,
    "Path-style": storage.force_path_style ? "Enabled" : "Disabled",
    Relay: storage.force_relay ? "Enabled" : "Disabled",
    "Configured capacity": bytes(storage.capacity_bytes),
  };
}
function Connection({
  id,
  revision,
  refreshing,
}: {
  id: string;
  revision: number;
  refreshing: boolean;
}) {
  const test = useQuery({
    queryKey: ["connection", id, revision],
    queryFn: ({ signal }) => command("storage.test", { id }, signal),
    enabled: false,
    retry: false,
    gcTime: 0,
  });
  return (
    <Stack component="section" aria-label="Storage connection" spacing={2}>
      <Stack
        direction="row"
        sx={{ justifyContent: "space-between", alignItems: "center" }}
      >
        <Typography component="h2" variant="h6">
          Connection
        </Typography>
        <Button
          startIcon={<PlugZap size={18} />}
          disabled={test.isFetching || refreshing}
          onClick={() => void test.refetch()}
        >
          {test.isFetching ? "Testing..." : "Test connection"}
        </Button>
      </Stack>
      {test.isFetching ? (
        <Typography role="status">Checking bucket access...</Typography>
      ) : test.error ? (
        <Alert severity="error">{message(test.error)}</Alert>
      ) : test.isSuccess ? (
        <Alert severity="success" role="status">
          Bucket access verified. Upload, download and public URL not tested.
        </Alert>
      ) : (
        <Typography variant="body2" color="text.secondary">
          Not checked
        </Typography>
      )}
    </Stack>
  );
}
function ServiceKeys({ id, writable }: { id: string; writable: boolean }) {
  const query = useQuery({
    queryKey: ["clients", "keys", id],
    queryFn: ({ signal }) =>
      command<string[]>("credential.list", { client_id: id }, signal),
  });
  const [creating, setCreating] = useState(false);
  const [revoke, setRevoke] = useState<string | null>(null);
  const [issued, setIssued] = useState<{
    access_key_id: string;
    secret_key: string;
  } | null>(null);
  const action = useAction();
  return (
    <Stack component="section" aria-label="S3 credentials" spacing={2}>
      <Stack
        direction="row"
        sx={{ justifyContent: "space-between", alignItems: "center" }}
      >
        <Typography variant="h6" component="h2">
          S3 credentials
        </Typography>
        {writable && (
          <Button
            variant="outlined"
            startIcon={<Plus size={18} />}
            onClick={() => setCreating(true)}
          >
            Create credential
          </Button>
        )}
      </Stack>
      <QueryState
        pending={query.isPending}
        error={query.error}
        retry={() => void query.refetch()}
      />
      {!query.isError &&
        query.data?.map((key) => (
          <Stack
            key={key}
            direction="row"
            spacing={2}
            sx={{ alignItems: "center", justifyContent: "space-between" }}
          >
            <Typography variant="body2" sx={{ overflowWrap: "anywhere" }}>
              {key}
            </Typography>
            {writable && (
              <Button color="error" onClick={() => setRevoke(key)}>
                Revoke {key}
              </Button>
            )}
          </Stack>
        ))}
      {!query.isError && query.data?.length === 0 && (
        <Typography variant="body2" color="text.secondary">
          No credentials issued.
        </Typography>
      )}
      {creating && !issued && (
        <FormDialog
          title="Create S3 credential"
          action={action}
          button="Create"
          onClose={() => setCreating(false)}
          submit={async () => {
            setIssued(await command("credential.create", { client_id: id }));
            void query.refetch();
          }}
        >
          <Typography>Issue an access key and secret key for {id}.</Typography>
        </FormDialog>
      )}
      {issued && (
        <SecretDialog
          title="Save S3 credential"
          values={{
            "Access key ID": issued.access_key_id,
            "Secret key": issued.secret_key,
          }}
          acknowledgement="I have saved this credential."
          onClose={() => {
            setIssued(null);
            setCreating(false);
          }}
        />
      )}
      {revoke && (
        <Confirmation
          title="Revoke S3 credential"
          text="Requests signed with this key will be rejected."
          confirm={revoke}
          onClose={() => setRevoke(null)}
          execute={async () => {
            await command("credential.delete", {
              client_id: id,
              access_key_id: revoke,
            });
            setRevoke(null);
            await query.refetch();
          }}
        />
      )}
    </Stack>
  );
}
