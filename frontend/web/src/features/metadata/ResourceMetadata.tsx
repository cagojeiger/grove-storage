import {
  DialogContent,
  DialogActions,
  TextField,
  IconButton,
  Button,
} from "@mui/material";

import { FormEvent, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, RefreshCw, Save } from "lucide-react";
import { command } from "../../api/commands";
import { ApiError, message } from "../../api/http";
import { Dialog } from "../../design/Dialog";
import { useAction } from "../access/useAction";
import { parseMetadata, ResourceMetadata as MetadataResult } from "./model";

type Props = { resource: "storage" | "client"; id: string; canWrite: boolean };

function metadataMessage(error: unknown): string {
  if (!(error instanceof ApiError) || error.outcome !== "not_applied")
    return "The outcome is unknown. Close and review the saved metadata before making another change.";
  if (error.status === 400) return "Check the metadata JSON and its size.";
  if (error.status === 404) return "This resource is no longer available.";
  return message(error);
}

export function ResourceMetadata({ resource, id, canWrite }: Props) {
  const [editing, setEditing] = useState(false);
  const query = useQuery({
    queryKey: [resource === "storage" ? "storages" : "clients", "metadata", id],
    queryFn: ({ signal }) =>
      command<MetadataResult>(`${resource}.metadata.show`, { id }, signal),
  });
  return (
    <div className="resource-metadata">
      <dt className="metadata-heading">
        <span>Metadata</span>
        <div className="page-actions">
          <IconButton
            type="submit"
            className="icon-button"
            title="Refresh metadata"
            aria-label="Refresh metadata"
            disabled={query.isFetching}
            onClick={() => void query.refetch()}
          >
            <RefreshCw size={18} />
          </IconButton>
          {canWrite && (
            <IconButton
              type="submit"
              className="icon-button"
              title="Edit metadata"
              aria-label="Edit metadata"
              disabled={!query.data || query.isError || query.isFetching}
              onClick={() => setEditing(true)}
            >
              <Pencil size={18} />
            </IconButton>
          )}
        </div>
      </dt>
      <dd>
        {query.isPending ? (
          <p role="status">Loading metadata...</p>
        ) : query.isError ? (
          <p role="alert">{message(query.error)}</p>
        ) : (
          <pre className="metadata-json">
            {JSON.stringify(query.data.metadata, null, 2)}
          </pre>
        )}
        {editing && canWrite && query.data && (
          <MetadataEditor
            resource={resource}
            id={id}
            value={query.data}
            onClose={() => {
              setEditing(false);
              void query.refetch();
            }}
          />
        )}
      </dd>
    </div>
  );
}

function MetadataEditor({
  resource,
  id,
  value,
  onClose,
}: Omit<Props, "canWrite"> & { value: MetadataResult; onClose: () => void }) {
  const cache = useQueryClient();
  const [text, setText] = useState(JSON.stringify(value.metadata, null, 2));
  const [validation, setValidation] = useState("");
  const state = useAction(metadataMessage);
  async function save(event: FormEvent) {
    event.preventDefault();
    if (state.busy || state.unknown) return;
    let metadata;
    try {
      metadata = parseMetadata(text);
    } catch (error) {
      setValidation((error as Error).message);
      return;
    }
    setValidation("");
    await state.run(async () => {
      const result = await command<MetadataResult>(
        `${resource}.metadata.replace`,
        { id, metadata },
      );
      cache.setQueryData(
        [resource === "storage" ? "storages" : "clients", "metadata", id],
        result,
      );
      onClose();
    });
  }
  return (
    <Dialog title="Edit metadata" busy={state.busy} onClose={onClose}>
      <form onSubmit={(event) => void save(event)}>
        <DialogContent>
          <TextField
            value={text}
            onChange={(event) => {
              setText(event.target.value);
              setValidation("");
            }}
            rows={12}
            autoComplete="off"
            disabled={state.busy || state.unknown}
            label="Metadata JSON"
            error={Boolean(validation)}
            helperText={validation}
            multiline
            slotProps={{
              htmlInput: { spellCheck: false },
              input: { sx: { fontFamily: "monospace" } },
              formHelperText: { role: validation ? "alert" : undefined },
            }}
          />
          {state.error && <p role="alert">{state.error}</p>}
        </DialogContent>
        <DialogActions>
          <Button type="button" disabled={state.busy} onClick={onClose}>
            {state.unknown ? "Close and review" : "Cancel"}
          </Button>
          <Button
            variant="contained"
            className="primary"
            type="submit"
            disabled={state.busy || state.unknown}
          >
            <Save size={16} />
            {state.busy ? "Saving..." : "Save"}
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  );
}
