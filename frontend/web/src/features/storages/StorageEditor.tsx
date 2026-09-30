import { Box, Button, Typography } from "@mui/material";
import { Input, Select } from "../../design/Fields";
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
      await command(storage ? "storage.replace" : "storage.create", { id, spec: body });
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
    <Box sx={{ maxWidth: 760 }}>
      <Button variant="text" disabled={busy} startIcon={<ArrowLeft size={16} />} onClick={onClose} sx={{ mb: 2 }}>Storage</Button>
      <Typography variant="h1" sx={{ mb: 3 }}>{storage ? "Edit storage" : "Register storage"}</Typography>
      <form id={formId} onSubmit={(event) => void save(event)} autoComplete="off">
        <fieldset disabled={busy || unknown} className="storage-form">
          <Typography component="h2" variant="h2" className="full-field">Connection</Typography>
          <label>
            Storage ID
            <Input
              name="id"
              required
              pattern={idPattern}
              maxLength={64}
              defaultValue={storage?.id}
              readOnly={Boolean(storage)}
              title="1-64 lowercase letters, digits, or hyphens"
            />
          </label>
          <label className="full-field">
            Endpoint
            <Input
              key="endpoint"
              name="endpoint"
              type="url"
              required
              defaultValue={storage?.endpoint ?? ""}
              placeholder="https://s3.example.com"
              spellCheck={false}
            />
          </label>
          <label className="full-field">
            Public endpoint (optional)
            <Input
              name="public_endpoint"
              type="url"
              defaultValue={storage?.public_endpoint ?? ""}
              spellCheck={false}
            />
          </label>
          <label>
            Region
            <Input
              name="region"
              required
              defaultValue={storage?.region ?? "us-east-1"}
            />
          </label>
          <label>
            Bucket
            <Input
              name="bucket"
              required
              defaultValue={storage?.bucket ?? ""}
              spellCheck={false}
            />
          </label>
          <Typography component="h2" variant="h2" className="full-field">Credentials</Typography>
          <label className="full-field">
            Access key
            <Input
              name="access_key"
              required
              defaultValue={storage?.access_key ?? ""}
              spellCheck={false}
            />
          </label>
          <label className="full-field">
            {storage ? "Secret key (re-enter)" : "Secret key"}
            <Input
              name="secret_key"
              type="password"
              required
              autoComplete="new-password"
            />
          </label>
          <Typography component="h2" variant="h2" className="full-field">Transfer and allocation</Typography>
          <label className="check-field">
            <Input
              name="force_path_style"
              type="checkbox"
              defaultChecked={storage?.force_path_style}
            />
            Path-style
          </label>
          <label className="check-field">
            <Input
              name="force_relay"
              type="checkbox"
              defaultChecked={storage?.force_relay}
            />
            Use relay
          </label>
          <label className="full-field">
            Registered capacity
            <div className="capacity-input">
              <Input
                name="capacity"
                aria-label="Registered capacity"
                inputMode="decimal"
                required
                defaultValue={storage?.capacity_bytes ?? "0"}
              />
              <Select name="unit" aria-label="Capacity unit" defaultValue="B">
                <option>B</option>
                <option>GiB</option>
                <option>TiB</option>
              </Select>
            </div>
          </label>
        </fieldset>
        {error && (
          <p role="alert" className="form-error">
            {error}
          </p>
        )}
      </form>
      <Box sx={{ display: "flex", justifyContent: "flex-end", gap: 1, py: 2, mt: 3, borderTop: 1, borderColor: "divider", position: "sticky", bottom: 0, bgcolor: "background.default", zIndex: 1 }}>
        <Button type="button" disabled={busy} onClick={onClose}>{unknown ? "Close and review" : "Cancel"}</Button>
        <Button variant="contained" type="submit" form={formId} disabled={busy || unknown}><Save size={16} />{busy ? "Saving..." : "Save"}</Button>
      </Box>
    </Box>
  );
}
