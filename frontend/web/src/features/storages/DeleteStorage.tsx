import { FormEvent, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { ApiError } from "../../api/http";
import { command } from "../../api/commands";
import { clearSession } from "../../auth/session";
import { Dialog } from "../../design/Dialog";
import { mutationMessage, refreshStorages, uncertain } from "./model";

export function DeleteStorage({
  id,
  onClose,
  onReturnToList,
}: {
  id: string;
  onClose: () => void;
  onReturnToList: () => void;
}) {
  const cache = useQueryClient();
  const pending = useRef(false);
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [unknown, setUnknown] = useState(false);
  async function remove(event: FormEvent) {
    event.preventDefault();
    if (pending.current || confirmation !== id || unknown) return;
    pending.current = true;
    setBusy(true);
    setError("");
    try {
      await command("storage.delete", { id });
      cache.removeQueries({ queryKey: ["storages", "detail", id] });
      await refreshStorages(cache);
      onReturnToList();
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        clearSession(cache);
        return;
      }
      setError(mutationMessage(error, "delete"));
      if (error instanceof ApiError && error.status === 403)
        void cache.invalidateQueries({ queryKey: ["session"] });
      setUnknown(uncertain(error));
      setConfirmation("");
      await refreshStorages(cache);
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }
  return (
    <Dialog title="Delete storage" busy={busy} onClose={onClose}>
      <form onSubmit={(event) => void remove(event)}>
        <p className="delete-summary">
          Remove <strong>{id}</strong> from the registry. The bucket or filesystem
          is retained.
        </p>
        <label className="confirmation">
          Storage ID to delete
          <input
            value={confirmation}
            onChange={(event) => setConfirmation(event.target.value)}
            disabled={busy || unknown}
            autoComplete="off"
          />
        </label>
        {error && (
          <p role="alert" className="form-error">
            {error}
          </p>
        )}
        <div className="dialog-actions">
          <button
            type="button"
            onClick={unknown ? onReturnToList : onClose}
            disabled={busy}
          >
            {unknown ? "Review list" : "Cancel"}
          </button>
          <button
            type="submit"
            className="danger action-button"
            disabled={busy || unknown || confirmation !== id}
          >
            <Trash2 size={16} />
            {busy ? "Deleting..." : "Confirm delete"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
