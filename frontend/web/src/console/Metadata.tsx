import { useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Alert,
  Button,
  Chip,
  Stack,
  Typography,
} from "@mui/material";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { command } from "../api/commands";
import {
  parseMetadata,
  type ResourceMetadata,
} from "../features/metadata/model";
import { useAction } from "../hooks/useAction";
import { FormDialog, QueryState } from "./ui";

export function Metadata({
  kind,
  id,
  writable,
}: {
  kind: "storage" | "client";
  id: string;
  writable: boolean;
}) {
  const cache = useQueryClient();
  const query = useQuery({
    queryKey: [kind, "metadata", id],
    queryFn: ({ signal }) =>
      command<ResourceMetadata>(`${kind}.metadata.show`, { id }, signal),
  });
  const [editing, setEditing] = useState(false);
  return (
    <Stack component="section" aria-label="Metadata" spacing={2}>
      <Stack
        direction="row"
        sx={{ justifyContent: "space-between", alignItems: "center" }}
      >
        <Typography component="h2" variant="h6">
          Metadata
        </Typography>
        {writable && (
          <Button
            disabled={!query.data || query.isError || query.isFetching}
            onClick={() => setEditing(true)}
          >
            Edit metadata
          </Button>
        )}
      </Stack>
      <QueryState
        pending={query.isPending}
        error={query.error}
        retry={() => void query.refetch()}
      />
      {!query.isError && query.data && (
        <Stack
          aria-label="Saved metadata"
          direction="row"
          spacing={1}
          useFlexGap
          sx={{ flexWrap: "wrap" }}
        >
          {Object.entries(query.data.metadata).length ? (
            Object.entries(query.data.metadata).map(([key, value]) => (
              <Chip
                key={key}
                variant="outlined"
                label={`${key}: ${value}`}
                sx={{
                  height: "auto",
                  "& .MuiChip-label": {
                    whiteSpace: "normal",
                    overflowWrap: "anywhere",
                    py: 0.5,
                  },
                }}
              />
            ))
          ) : (
            <Typography variant="body2" color="text.secondary">
              No metadata.
            </Typography>
          )}
        </Stack>
      )}
      {editing && query.data && (
        <MetadataEditor
          data={query.data}
          kind={kind}
          onClose={() => {
            void query.refetch().then(() => setEditing(false));
          }}
          onSaved={async () => {
            await cache.invalidateQueries({ queryKey: [kind, "metadata", id] });
            setEditing(false);
          }}
        />
      )}
    </Stack>
  );
}
function MetadataEditor({
  data,
  kind,
  onClose,
  onSaved,
}: {
  data: ResourceMetadata;
  kind: "storage" | "client";
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const action = useAction();
  const [value, setValue] = useState(JSON.stringify(data.metadata, null, 2));
  const [error, setError] = useState("");
  return (
    <FormDialog
      title="Edit metadata"
      onClose={onClose}
      action={action}
      submit={async () => {
        let metadata;
        try {
          metadata = parseMetadata(value);
        } catch (failure) {
          setError(
            failure instanceof Error ? failure.message : "Invalid metadata.",
          );
          return;
        }
        await command(`${kind}.metadata.replace`, { id: data.id, metadata });
        await onSaved();
      }}
    >
      <TextField
        label="Metadata JSON"
        fullWidth
        multiline
        minRows={6}
        value={value}
        error={Boolean(error)}
        helperText={error || undefined}
        onChange={(event) => {
          setValue(event.target.value);
          setError("");
        }}
        spellCheck={false}
      />
      {error && <Alert severity="error">{error}</Alert>}
    </FormDialog>
  );
}
