import { FormEvent, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { admin, ApiError, request } from "../../api/http";
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
      await request(`${admin}/storages/${encodeURIComponent(id)}`, {
        method: "DELETE",
      });
      cache.removeQueries({ queryKey: ["storages", "detail", id] });
      await refreshStorages(cache);
      onReturnToList();
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        clearSession(cache);
        return;
      }
      setError(mutationMessage(error, "delete"));
      setUnknown(uncertain(error));
      setConfirmation("");
      await refreshStorages(cache);
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }
  return (
    <Dialog title="저장소 삭제" busy={busy} onClose={onClose}>
      <form onSubmit={(event) => void remove(event)}>
        <p className="delete-summary">
          <strong>{id}</strong> 등록을 삭제합니다. 실제 버킷·파일시스템은
          유지됩니다.
        </p>
        <label className="confirmation">
          삭제할 저장소 ID
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
            {unknown ? "목록에서 확인" : "취소"}
          </button>
          <button
            type="submit"
            className="danger action-button"
            disabled={busy || unknown || confirmation !== id}
          >
            <Trash2 size={16} />
            {busy ? "삭제 중..." : "삭제 확인"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
