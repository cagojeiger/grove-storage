import { FormEvent, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Copy } from "lucide-react";
import { command } from "../../api/commands";
import { field } from "../../api/identity";
import { Dialog } from "../../design/Dialog";
import { useAction } from "../access/useAction";
import { clientMessage } from "./model";

export type KeyAction =
  | { kind: "native-generate" | "native-register" | "s3-create" }
  | { kind: "native-delete" | "s3-delete"; key: string };
type IssuedKey = { name: string; value: string }[];
export function KeyDialog({
  clientId,
  action,
  onClose,
}: {
  clientId: string;
  action: KeyAction;
  onClose: () => void;
}) {
  const state = useAction(clientMessage);
  const cache = useQueryClient();
  const [issued, setIssued] = useState<IssuedKey | null>(null);
  const [saved, setSaved] = useState(false);
  const [confirmation, setConfirmation] = useState("");
  const [copyError, setCopyError] = useState("");
  const deleting = "key" in action;
  const title = {
    "native-generate": "Generate Native key",
    "native-register": "Register Native key",
    "s3-create": "Issue S3 key",
    "native-delete": "Revoke Native key",
    "s3-delete": "Revoke S3 key",
  }[action.kind];
  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    if (deleting && confirmation !== clientId) return;
    const data = new FormData(e.currentTarget);
    const input = e.currentTarget.elements.namedItem("key");
    if (input instanceof HTMLInputElement) input.value = "";
    await state.run(async () => {
      if (action.kind === "s3-create") {
        const result = await command<{
          access_key_id: string;
          secret_key: string;
        }>("credential.create", { client_id: clientId });
        setIssued([
          { name: "Access key ID", value: result.access_key_id },
          { name: "Secret key", value: result.secret_key },
        ]);
      } else if (
        action.kind === "native-generate" ||
        action.kind === "native-register"
      ) {
        const raw =
          action.kind === "native-register"
            ? field(data, "key").trim()
            : "gsk_" +
              Array.from(crypto.getRandomValues(new Uint8Array(32)), (b) =>
                b.toString(16).padStart(2, "0"),
              ).join("");
        const digest = await crypto.subtle.digest(
          "SHA-256",
          new TextEncoder().encode(raw),
        );
        const key_hash =
          "sha256:" +
          Array.from(new Uint8Array(digest), (b) =>
            b.toString(16).padStart(2, "0"),
          ).join("");
        await command("client-key.register", { client_id: clientId, key_hash });
        if (action.kind === "native-generate")
          setIssued([{ name: "Native API key", value: raw }]);
        else onClose();
      } else if ("key" in action) {
        await command(
          action.kind === "s3-delete"
            ? "credential.delete"
            : "client-key.delete",
          {
            client_id: clientId,
            ...(action.kind === "s3-delete"
              ? { access_key_id: action.key }
              : { key_hash: action.key }),
          },
        );
        onClose();
      }
      await cache.invalidateQueries({
        queryKey: ["clients", "keys", clientId],
      });
    });
  }
  return (
    <Dialog
      title={title}
      busy={state.busy}
      closeDisabled={Boolean(issued) && !saved}
      onClose={onClose}
    >
      {issued ? (
        <div className="issued-token">
          {issued.map((item) => (
            <label key={item.name}>
              {item.name}
              <input
                readOnly
                value={item.value}
                autoComplete="off"
                spellCheck={false}
              />
              <button
                type="button"
                className="icon-button"
                title={`Copy ${item.name}`}
                aria-label={`Copy ${item.name}`}
                onClick={() => {
                  setCopyError("");
                  void navigator.clipboard
                    .writeText(item.value)
                    .catch(() =>
                      setCopyError("Copy failed. Select the key to copy it."),
                    );
                }}
              >
                <Copy size={16} />
              </button>
            </label>
          ))}
          {copyError && <p role="alert">{copyError}</p>}
          <label className="check-field">
            <input
              type="checkbox"
              checked={saved}
              onChange={(e) => setSaved(e.target.checked)}
            />
            I have saved these keys. Secrets are shown only once.
          </label>
          <button className="primary" disabled={!saved} onClick={onClose}>
            Done
          </button>
        </div>
      ) : (
        <form onSubmit={(e) => void submit(e)}>
          <fieldset
            className="storage-form"
            disabled={state.busy || state.unknown}
          >
            <p className="full-field">
              Client: <strong>{clientId}</strong>
            </p>
            {action.kind === "native-register" && (
              <label className="full-field">
                Existing Native key
                <input
                  name="key"
                  type="password"
                  autoComplete="off"
                  required
                  pattern=".*\S.*"
                />
              </label>
            )}
            {deleting && (
              <>
                <p className="full-field key-value">{action.key}</p>
                <label className="full-field">
                  Client ID to confirm
                  <input
                    autoComplete="off"
                    required
                    value={confirmation}
                    onChange={(e) => setConfirmation(e.target.value)}
                  />
                </label>
              </>
            )}
          </fieldset>
          {state.error && (
            <p className="form-error" role="alert">
              {state.error}
            </p>
          )}
          <div className="dialog-actions">
            <button type="button" disabled={state.busy} onClick={onClose}>
              {state.unknown ? "Close and review" : "Cancel"}
            </button>
            <button
              className={deleting ? "danger" : "primary"}
              disabled={
                state.busy ||
                state.unknown ||
                (deleting && confirmation !== clientId)
              }
            >
              {state.busy ? "Saving..." : "Confirm"}
            </button>
          </div>
        </form>
      )}
    </Dialog>
  );
}
