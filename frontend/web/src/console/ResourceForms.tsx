import { useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Alert,
  Checkbox,
  FormControlLabel,
  Stack,
  Typography,
} from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import { command } from "../api/commands";
import { field } from "../api/identity";
import { useAction } from "../hooks/useAction";
import {
  Storage,
  idPattern,
  mutationMessage,
  storageSpec,
} from "../features/storages/model";
import { capacityInput } from "../features/storages/capacity";
import { clientMessage } from "../features/clients/model";
import { FormDialog, QueryState } from "./ui";

export function StorageForm({
  storage,
  onClose,
  onSaved,
}: {
  storage?: Storage;
  onClose: () => void;
  onSaved: (id: string) => Promise<void>;
}) {
  const action = useAction((error) =>
    mutationMessage(error, storage ? "replace" : "create"),
  );
  const [validation, setValidation] = useState("");
  const capacity = capacityInput(storage?.capacity_bytes ?? 0);
  return (
    <FormDialog
      title={storage ? "Edit storage" : "Add storage"}
      onClose={onClose}
      action={action}
      submit={async (data, form) => {
        let spec;
        try {
          spec = storageSpec(data, "s3");
        } catch (error) {
          setValidation(
            error instanceof Error ? error.message : "Invalid settings.",
          );
          return;
        }
        const secret = form.elements.namedItem("secret_key");
        if (secret instanceof HTMLInputElement) secret.value = "";
        const id = storage?.id ?? field(data, "id");
        await command(storage ? "storage.replace" : "storage.create", {
          id,
          spec,
        });
        await onSaved(id);
      }}
    >
      <TextField
        fullWidth
        label="Storage ID"
        name="id"
        required
        defaultValue={storage?.id}
        slotProps={{
          input: { readOnly: Boolean(storage) },
          htmlInput: { pattern: idPattern, maxLength: 64 },
        }}
      />
      <TextField
        fullWidth
        label="Endpoint"
        name="endpoint"
        type="url"
        required
        defaultValue={storage?.endpoint ?? ""}
      />
      <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
        <TextField
          fullWidth
          label="Region"
          name="region"
          required
          defaultValue={storage?.region ?? "us-east-1"}
        />
        <TextField
          fullWidth
          label="Bucket"
          name="bucket"
          required
          defaultValue={storage?.bucket ?? ""}
        />
      </Stack>
      <TextField
        fullWidth
        label="Public endpoint (optional)"
        name="public_endpoint"
        type="url"
        defaultValue={storage?.public_endpoint ?? ""}
      />
      <Typography variant="subtitle2">Provider credentials</Typography>
      <TextField
        fullWidth
        label="Access key"
        name="access_key"
        required
        defaultValue={storage?.access_key ?? ""}
        autoComplete="off"
      />
      <TextField
        fullWidth
        label={storage ? "Secret key (re-enter)" : "Secret key"}
        name="secret_key"
        required
        type="password"
        autoComplete="new-password"
      />
      <Stack direction="row" spacing={2}>
        <TextField
          fullWidth
          label="Configured capacity"
          name="capacity"
          required
          defaultValue={capacity.value}
          slotProps={{
            htmlInput: {
              inputMode: "decimal",
              "aria-label": "Configured capacity",
            },
          }}
        />
        <TextField
          label="Capacity unit"
          name="unit"
          select
          slotProps={{ select: { native: true } }}
          defaultValue={capacity.unit}
          sx={{ minWidth: 100 }}
        >
          {["B", "GiB", "TiB"].map((unit) => (
            <option key={unit} value={unit}>
              {unit}
            </option>
          ))}
        </TextField>
      </Stack>
      <FormControlLabel
        label="Path-style"
        control={
          <Checkbox
            name="force_path_style"
            defaultChecked={storage?.force_path_style}
          />
        }
      />
      <FormControlLabel
        label="Use relay"
        control={
          <Checkbox name="force_relay" defaultChecked={storage?.force_relay} />
        }
      />
      {validation && <Alert severity="error">{validation}</Alert>}
    </FormDialog>
  );
}
export function ClientForm({
  onClose,
  onSaved,
}: {
  onClose: () => void;
  onSaved: (id: string) => Promise<void>;
}) {
  const action = useAction(clientMessage);
  const query = useQuery({
    queryKey: ["storages", "list"],
    queryFn: ({ signal }) => command<Storage[]>("storage.list", {}, signal),
  });
  const options = query.data?.filter((storage) => storage.kind === "s3") ?? [];
  return (
    <FormDialog
      title="Create client"
      onClose={onClose}
      action={action}
      button="Create"
      disabled={query.isError || !options.length}
      submit={async (data) => {
        const id = field(data, "id").trim();
        await command("client.create", {
          id,
          storage_id: field(data, "storage_id"),
        });
        await onSaved(id);
      }}
    >
      <TextField
        fullWidth
        label="Client ID"
        name="id"
        required
        autoComplete="off"
        slotProps={{ htmlInput: { pattern: idPattern, maxLength: 64 } }}
      />
      <TextField
        fullWidth
        select
        slotProps={{ select: { native: true } }}
        label="Storage"
        name="storage_id"
        required
        defaultValue=""
        disabled={query.isPending || query.isError}
      >
        {options.map((storage) => (
          <option key={storage.id} value={storage.id}>
            {storage.id}
          </option>
        ))}
      </TextField>
      <QueryState
        pending={query.isPending}
        error={query.error}
        retry={() => void query.refetch()}
      />
      {!query.isPending && !query.isError && !options.length && (
        <Alert severity="info">No S3 storage registered.</Alert>
      )}
    </FormDialog>
  );
}
