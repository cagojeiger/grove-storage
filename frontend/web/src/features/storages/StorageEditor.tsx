import { FormEvent, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Save } from "lucide-react";
import { admin, ApiError, request } from "../../api/http";
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
        error instanceof Error ? error.message : "입력값을 확인해 주세요.",
      );
      return;
    }
    const secret = form.elements.namedItem("secret_key");
    if (secret instanceof HTMLInputElement) secret.value = "";
    pending.current = true;
    setBusy(true);
    setError("");
    try {
      await request(
        `${admin}/storages${storage ? `/${encodeURIComponent(id)}` : ""}`,
        {
          method: storage ? "PUT" : "POST",
          body: JSON.stringify(storage ? body : { id, ...body }),
        },
      );
      await refreshStorages(cache);
      onSaved(id);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        clearSession(cache);
        return;
      }
      setError(mutationMessage(error, storage ? "replace" : "create"));
      setUnknown(uncertain(error));
      await refreshStorages(cache);
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }
  return (
    <Dialog
      title={storage ? "저장소 수정" : "저장소 등록"}
      busy={busy}
      onClose={onClose}
    >
      <form onSubmit={(event) => void save(event)} autoComplete="off">
        <fieldset disabled={busy || unknown} className="storage-form">
          <label>
            저장소 ID
            <input
              name="id"
              required
              pattern={idPattern}
              maxLength={64}
              defaultValue={storage?.id}
              readOnly={Boolean(storage)}
              title="소문자, 숫자, 하이픈으로 구성된 1~64자 ID"
            />
          </label>
          <label>
            종류
            <select
              name="kind"
              aria-label="종류"
              value={kind}
              onChange={(event) =>
                setKind(event.target.value as Storage["kind"])
              }
            >
              <option value="s3">S3</option>
              <option value="fs">파일시스템</option>
            </select>
          </label>
          {kind === "fs" ? (
            <label className="full-field">
              루트 경로
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
                Public endpoint (선택)
                <input
                  name="public_endpoint"
                  type="url"
                  defaultValue={storage?.public_endpoint ?? ""}
                  spellCheck={false}
                />
              </label>
              <label>
                리전
                <input
                  name="region"
                  required
                  defaultValue={storage?.region ?? "us-east-1"}
                />
              </label>
              <label>
                버킷
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
                {storage ? "Secret key (재입력)" : "Secret key"}
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
                릴레이 사용
              </label>
            </>
          )}
          <label className="full-field">
            등록 용량
            <div className="capacity-input">
              <input
                name="capacity"
                aria-label="등록 용량"
                inputMode="decimal"
                required
                defaultValue={storage?.capacity_bytes ?? "0"}
              />
              <select name="unit" aria-label="용량 단위" defaultValue="B">
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
            {unknown ? "닫고 확인" : "취소"}
          </button>
          <button className="primary" type="submit" disabled={busy || unknown}>
            <Save size={16} />
            {busy ? "저장 중..." : "저장"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
