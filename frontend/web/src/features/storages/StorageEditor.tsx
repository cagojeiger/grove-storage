import { FormEvent, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Save } from "lucide-react";
import { ApiError } from "../../api/http";
import { command } from "../../api/commands";
import { clearSession } from "../../auth/session";
import { Dialog } from "../../design/Dialog";
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
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [kind, setKind] = useState<Storage["kind"]>(storage?.kind ?? "s3");
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
      body = storageSpec(data, kind);
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
    <Dialog
      title={storage ? "Edit storage" : "Register storage"}
      busy={busy}
      onClose={onClose}
    >
      <form onSubmit={(event) => void save(event)} autoComplete="off">
        <fieldset disabled={busy || unknown} className="storage-form">
          <label>
            Storage ID
            <input
              name="id"
              required
              pattern={idPattern}
              maxLength={64}
              defaultValue={storage?.id}
              readOnly={Boolean(storage)}
              title="1-64 lowercase letters, digits, or hyphens"
            />
          </label>
          <label>
            Type
            <select
              name="kind"
              aria-label="Type"
              value={kind}
              onChange={(event) =>
                setKind(event.target.value as Storage["kind"])
              }
            >
              <option value="s3">S3</option>
              <option value="fs">Filesystem</option>
            </select>
          </label>
          {kind === "fs" ? (
            <label className="full-field">
              Root path
              <input
                key="root_path"
                name="root_path"
                required
                defaultValue={storage?.root_path ?? ""}
                spellCheck={false}
              />
            </label>
          ) : (
            <>
              <label className="full-field">
                Endpoint
                <input
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
                <input
                  name="public_endpoint"
                  type="url"
                  defaultValue={storage?.public_endpoint ?? ""}
                  spellCheck={false}
                />
              </label>
              <label>
                Region
                <input
                  name="region"
                  required
                  defaultValue={storage?.region ?? "us-east-1"}
                />
              </label>
              <label>
                Bucket
                <input
                  name="bucket"
                  required
                  defaultValue={storage?.bucket ?? ""}
                  spellCheck={false}
                />
              </label>
              <label className="full-field">
                Access key
                <input
                  name="access_key"
                  required
                  defaultValue={storage?.access_key ?? ""}
                  spellCheck={false}
                />
              </label>
              <label className="full-field">
                {storage ? "Secret key (re-enter)" : "Secret key"}
                <input
                  name="secret_key"
                  type="password"
                  required
                  autoComplete="new-password"
                />
              </label>
              <label className="check-field">
                <input
                  name="force_path_style"
                  type="checkbox"
                  defaultChecked={storage?.force_path_style}
                />
                Path-style
              </label>
              <label className="check-field">
                <input
                  name="force_relay"
                  type="checkbox"
                  defaultChecked={storage?.force_relay}
                />
                Use relay
              </label>
            </>
          )}
          <label className="full-field">
            Registered capacity
            <div className="capacity-input">
              <input
                name="capacity"
                aria-label="Registered capacity"
                inputMode="decimal"
                required
                defaultValue={storage?.capacity_bytes ?? "0"}
              />
              <select name="unit" aria-label="Capacity unit" defaultValue="B">
                <option>B</option>
                <option>GiB</option>
                <option>TiB</option>
              </select>
            </div>
          </label>
        </fieldset>
        {error && (
          <p role="alert" className="form-error">
            {error}
          </p>
        )}
        <div className="dialog-actions">
          <button type="button" disabled={busy} onClick={onClose}>
            {unknown ? "Close and review" : "Cancel"}
          </button>
          <button className="primary" type="submit" disabled={busy || unknown}>
            <Save size={16} />
            {busy ? "Saving..." : "Save"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
