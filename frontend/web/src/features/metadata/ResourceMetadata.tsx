import {
  DialogContent,
  Dialog,
  DialogTitle,
  DialogActions,
  TextField,
  IconButton,
  Button,
  Alert,
  Box,
  Stack,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { FormEvent, useId, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, RefreshCw, Save } from "lucide-react";
import { command } from "../../api/commands";
import { ApiError, message } from "../../api/http";
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
    <Stack component="section" aria-label="Metadata" spacing={2}>
      <Stack
        direction="row"
        sx={{
          alignItems: "center",
          justifyContent: "space-between",
          gap: 1,
        }}
      >
        <Typography component="h2" variant="h6">
          Metadata
        </Typography>
        <Stack direction="row" spacing={0.5}>
          <Tooltip title="Refresh metadata">
            <span>
              <IconButton
                type="button"
                aria-label="Refresh metadata"
                disabled={query.isFetching}
                onClick={() => void query.refetch()}
              >
                <RefreshCw size={18} />
              </IconButton>
            </span>
          </Tooltip>
          {canWrite && (
            <Tooltip title="Edit metadata">
              <span>
                <IconButton
                  type="button"
                  aria-label="Edit metadata"
                  disabled={!query.data || query.isError}
                  aria-disabled={query.isFetching || undefined}
                  onClick={() => {
                    if (!query.isFetching) setEditing(true);
                  }}
                >
                  <Pencil size={18} />
                </IconButton>
              </span>
            </Tooltip>
          )}
        </Stack>
      </Stack>
      {query.isPending ? (
        <Typography role="status" variant="body2">
          Loading metadata...
        </Typography>
      ) : query.isError ? (
        <Alert severity="error">{message(query.error)}</Alert>
      ) : (
        <Box
          component="pre"
          aria-label="Saved metadata"
          sx={{
            m: 0,
            typography: "body2",
            fontFamily: "monospace",
            whiteSpace: "pre-wrap",
            overflowWrap: "anywhere",
          }}
        >
          {JSON.stringify(query.data.metadata, null, 2)}
        </Box>
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
    </Stack>
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
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));
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
    <Dialog
      open
      fullWidth
      maxWidth="sm"
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !state.busy) onClose();
      }}
    >
      <DialogTitle id={titleId}>Edit metadata</DialogTitle>
      <DialogContent dividers>
        <form id={`${titleId}-form`} onSubmit={(event) => void save(event)}>
          <TextField
            value={text}
            onChange={(event) => {
              setText(event.target.value);
              setValidation("");
            }}
            minRows={4}
            maxRows={12}
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
          {state.error && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {state.error}
            </Alert>
          )}
        </form>
      </DialogContent>
      <DialogActions>
        <Button type="button" disabled={state.busy} onClick={onClose}>
          {state.unknown ? "Close and review" : "Cancel"}
        </Button>
        <Button
          variant="contained"
          startIcon={<Save size={16} />}
          type="submit"
          form={`${titleId}-form`}
          disabled={state.busy || state.unknown}
        >
          {state.busy ? "Saving..." : "Save"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
