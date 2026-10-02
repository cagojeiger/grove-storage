import {
  TextField,
  Checkbox,
  FormControlLabel,
  Box,
  Grid,
  Button,
  Typography,
  Alert,
  Divider,
  Stack,
} from "@mui/material";

import { FormEvent, useEffect, useId, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Save } from "lucide-react";
import { ApiError } from "../../api/http";
import { command } from "../../api/commands";
import { clearSession } from "../../auth/session";
import { capacityInput } from "./capacity";
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
  const titleId = useId();
  const capacity = capacityInput(storage?.capacity_bytes ?? 0);
  const pending = useRef(false);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
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
      if (mounted.current) onSaved(id);
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
    <Stack spacing={3}>
      <Box>
        <form
          id={`${titleId}-form`}
          onSubmit={(event) => void save(event)}
          autoComplete="off"
        >
          <Stack
            component="fieldset"
            disabled={busy || unknown}
            spacing={3}
            sx={{ m: 0, p: 0, border: 0, minWidth: 0 }}
          >
            <Grid container spacing={4}>
              <Grid size={{ xs: 12, md: 7 }}>
                <Stack spacing={3}>
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
                  <Grid container spacing={2}>
                    <Grid size={{ xs: 12, sm: 6 }}>
                      <TextField
                        name="region"
                        required
                        defaultValue={storage?.region ?? "us-east-1"}
                        label={"Region"}
                      />
                    </Grid>
                    <Grid size={{ xs: 12, sm: 6 }}>
                      <TextField
                        name="bucket"
                        required
                        defaultValue={storage?.bucket ?? ""}
                        label={"Bucket"}
                        slotProps={{ htmlInput: { spellCheck: false } }}
                      />
                    </Grid>
                  </Grid>
                </Stack>
              </Grid>
              <Grid size={{ xs: 12, md: 5 }}>
                <Stack spacing={3}>
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
                  <Grid container spacing={2}>
                    <Grid size={{ xs: 12, sm: 8 }}>
                      <TextField
                        label="Configured capacity"
                        name="capacity"
                        required
                        defaultValue={capacity.value}
                        slotProps={{
                          htmlInput: {
                            "aria-label": "Configured capacity",
                            inputMode: "decimal",
                          },
                        }}
                      />
                    </Grid>
                    <Grid size={{ xs: 12, sm: 4 }}>
                      <TextField
                        label="Unit"
                        name="unit"
                        defaultValue={capacity.unit}
                        select
                        slotProps={{
                          htmlInput: { "aria-label": "Capacity unit" },
                          select: { native: true },
                        }}
                      >
                        <option>B</option>
                        <option>GiB</option>
                        <option>TiB</option>
                      </TextField>
                    </Grid>
                  </Grid>
                </Stack>
              </Grid>
            </Grid>
          </Stack>
          {error && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {error}
            </Alert>
          )}
        </form>
      </Box>
      <Divider />
      <Stack direction="row" spacing={1} sx={{ justifyContent: "flex-end" }}>
        <Button type="button" disabled={busy} onClick={onClose}>
          {unknown ? "Close and review" : "Cancel"}
        </Button>
        <Button
          variant="contained"
          type="submit"
          form={`${titleId}-form`}
          startIcon={<Save size={16} />}
          disabled={busy || unknown}
        >
          {busy ? "Saving..." : "Save"}
        </Button>
      </Stack>
    </Stack>
  );
}
