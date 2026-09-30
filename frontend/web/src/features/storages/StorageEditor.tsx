import {
  TextField,
  Checkbox,
  FormControlLabel,
  Box,
  Container,
  Button,
  Typography,
  Alert,
  Divider,
  Stack,
} from "@mui/material";

import { FormEvent, useId, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Save } from "lucide-react";
import { ApiError } from "../../api/http";
import { command } from "../../api/commands";
import { clearSession } from "../../auth/session";
import {
  Storage,
  idPattern,
  mutationMessage,
  refreshStorages,
  storageSpec,
  uncertain,
} from "./model";

export function StorageEditor({
  storage,
  onClose,
  onSaved,
}: {
  storage?: Storage;
  onClose: () => void;
  onSaved: (id: string) => void;
}) {
  const cache = useQueryClient();
  const formId = useId();
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [unknown, setUnknown] = useState(false);
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending.current || unknown) return;
    const form = event.currentTarget;
    let body;
    let id;
    try {
      const data = new FormData(form);
      const identifier = data.get("id");
      id = storage?.id ?? (typeof identifier === "string" ? identifier : "");
      body = storageSpec(data, "s3");
    } catch (error) {
      setError(
        error instanceof Error ? error.message : "Check the input values.",
      );
      return;
    }
    const secret = form.elements.namedItem("secret_key");
    if (secret instanceof HTMLInputElement) secret.value = "";
    pending.current = true;
    setBusy(true);
    setError("");
    try {
      await command(storage ? "storage.replace" : "storage.create", {
        id,
        spec: body,
      });
      await refreshStorages(cache);
      onSaved(id);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        clearSession(cache);
        return;
      }
      setError(mutationMessage(error, storage ? "replace" : "create"));
      if (error instanceof ApiError && error.status === 403)
        void cache.invalidateQueries({ queryKey: ["session"] });
      setUnknown(uncertain(error));
      await refreshStorages(cache);
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }
  return (
    <Container maxWidth="md" disableGutters>
      <Button
        variant="text"
        disabled={busy}
        startIcon={<ArrowLeft size={16} />}
        onClick={onClose}
        sx={{ mb: 2 }}
      >
        Storage
      </Button>
      <Typography component="h1" variant="h5" sx={{ mb: 3 }}>
        {storage ? "Edit storage" : "Register storage"}
      </Typography>
      <form
        id={formId}
        onSubmit={(event) => void save(event)}
        autoComplete="off"
      >
        <Stack
          component="fieldset"
          disabled={busy || unknown}
          spacing={3}
          sx={{ m: 0, p: 0, border: 0, minWidth: 0 }}
        >
          <Typography component="h2" variant="h6">
            Connection
          </Typography>
          <TextField
            name="id"
            required
            defaultValue={storage?.id}
            label={"Storage ID"}
            slotProps={{
              htmlInput: {
                pattern: idPattern,
                maxLength: 64,
                title: "1-64 lowercase letters, digits, or hyphens",
              },
              input: { readOnly: Boolean(storage) },
            }}
          />
          <TextField
            key="endpoint"
            name="endpoint"
            type="url"
            required
            defaultValue={storage?.endpoint ?? ""}
            placeholder="https://s3.example.com"
            label={"Endpoint"}
            slotProps={{ htmlInput: { spellCheck: false } }}
          />
          <TextField
            name="public_endpoint"
            type="url"
            defaultValue={storage?.public_endpoint ?? ""}
            label={"Public endpoint (optional)"}
            slotProps={{ htmlInput: { spellCheck: false } }}
          />
          <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
            <TextField
              name="region"
              required
              defaultValue={storage?.region ?? "us-east-1"}
              label={"Region"}
            />
            <TextField
              name="bucket"
              required
              defaultValue={storage?.bucket ?? ""}
              label={"Bucket"}
              slotProps={{ htmlInput: { spellCheck: false } }}
            />
          </Stack>
          <Divider />
          <Typography component="h2" variant="h6">
            Credentials
          </Typography>
          <TextField
            name="access_key"
            required
            defaultValue={storage?.access_key ?? ""}
            label={"Access key"}
            slotProps={{ htmlInput: { spellCheck: false } }}
          />
          <TextField
            name="secret_key"
            type="password"
            required
            autoComplete="new-password"
            label={storage ? "Secret key (re-enter)" : "Secret key"}
          />
          <Divider />
          <Typography component="h2" variant="h6">
            Transfer and allocation
          </Typography>
          <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
            <FormControlLabel
              control={
                <Checkbox
                  name="force_path_style"
                  defaultChecked={storage?.force_path_style}
                />
              }
              label={"Path-style"}
            />
            <FormControlLabel
              control={
                <Checkbox
                  name="force_relay"
                  defaultChecked={storage?.force_relay}
                />
              }
              label={"Use relay"}
            />
          </Stack>
          <Stack direction="row" spacing={2}>
            <TextField
              label="Registered capacity"
              name="capacity"
              required
              defaultValue={storage?.capacity_bytes ?? "0"}
              slotProps={{
                htmlInput: {
                  "aria-label": "Registered capacity",
                  inputMode: "decimal",
                },
              }}
            />
            <TextField
              label="Unit"
              name="unit"
              defaultValue="B"
              select
              fullWidth={false}
              slotProps={{
                htmlInput: { "aria-label": "Capacity unit" },
                select: { native: true },
              }}
            >
              <option>B</option>
              <option>GiB</option>
              <option>TiB</option>
            </TextField>
          </Stack>
        </Stack>
        {error && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {error}
          </Alert>
        )}
      </form>
      <Box
        sx={{
          display: "flex",
          justifyContent: "flex-end",
          gap: 1,
          py: 2,
          mt: 3,
          borderTop: 1,
          borderColor: "divider",
          position: "sticky",
          bottom: 0,
          bgcolor: "background.default",
          zIndex: 1,
        }}
      >
        <Button type="button" disabled={busy} onClick={onClose}>
          {unknown ? "Close and review" : "Cancel"}
        </Button>
        <Button
          variant="contained"
          type="submit"
          form={formId}
          startIcon={<Save size={16} />}
          disabled={busy || unknown}
        >
          {busy ? "Saving..." : "Save"}
        </Button>
      </Box>
    </Container>
  );
}
