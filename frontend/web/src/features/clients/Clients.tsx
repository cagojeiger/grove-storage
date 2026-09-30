import { IconButton, Button } from "@mui/material";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Plus, RefreshCw, Trash2 } from "lucide-react";
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
import { ClientRows } from "./ClientRows";
import { ResourceMetadata } from "../metadata/ResourceMetadata";
import { paginate, useResourceList } from "../../app/resourceList";
import { ListToolbar, Pagination } from "../../design/ResourceList";

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
    <main className="overview storages clients">
      {id && (
        <a className="back-link" href={listing.href("#clients")}>
          <ArrowLeft size={16} />
          Clients
        </a>
      )}
      <div className="page-heading">
        <div>
          <p className="eyebrow">REGISTRY</p>
          <h1>{id || "Clients"}</h1>
        </div>
        <div className="page-actions">
          <IconButton type="submit"
            className="icon-button"
            title="Refresh clients"
            aria-label="Refresh clients"
            disabled={current.isFetching || usage.isFetching}
            onClick={() => void refreshClients(cache)}
          >
            <RefreshCw size={18} />
          </IconButton>
          {canWrite &&
            (id ? (
              <IconButton type="submit"
                className="icon-button danger"
                color="error"
                title="Delete client"
                aria-label="Delete client"
                disabled={!detail.data || current.isError}
                onClick={() => setDialog(true)}
              >
                <Trash2 size={18} />
              </IconButton>
            ) : (
              <Button type="submit" variant="contained" className="primary" onClick={() => setDialog(true)}>
                <Plus size={18} />
                Create client
              </Button>
            ))}
        </div>
      </div>
      {current.isPending ? (
        <p role="status">Loading clients...</p>
      ) : current.isError ? (
        <p role="alert">{message(current.error)}</p>
      ) : id && detail.data ? (
        <>
          <dl className="detail-fields">
            <div>
              <dt>Client ID</dt>
              <dd>{id}</dd>
            </div>
            <div>
              <dt>S3 bucket</dt>
              <dd>{id}</dd>
            </div>
            <div>
              <dt>Storage</dt>
              <dd>
                <a href={storageLink(detail.data.storage_id)}>
                  {detail.data.storage_id}
                </a>
              </dd>
            </div>
            <div>
              <dt>Active files</dt>
              <dd>
                {usage.isError || !used
                  ? "Unavailable"
                  : used.files.toLocaleString("en-US")}
              </dd>
            </div>
            <div>
              <dt>Active data</dt>
              <dd>
                {usage.isError || !used ? "Unavailable" : bytes(used.bytes)}
              </dd>
            </div>
            <ResourceMetadata key={`metadata:${id}`} resource="client" id={id} canWrite={canWrite} />
          </dl>
          {canWrite && <ClientKeys key={id} clientId={id} />}
        </>
      ) : (
        <>
          <ListToolbar state={listing} label="Search clients" />
          <ClientRows
            ids={page.rows}
            usage={usage.isSuccess ? usage.data : undefined}
            href={listing.href}
          />
          {page.total === 0 && (
            <p className="empty">
              {listing.search
                ? "No matching clients."
                : "No clients registered."}
            </p>
          )}
          <Pagination state={listing} {...page} />
        </>
      )}
      {usage.isError && (
        <p role="alert">Usage unavailable. {message(usage.error)}</p>
      )}
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
    </main>
  );
}
