import { Input } from "../../design/Fields";
import { Button } from "@mui/material";
import { FormEvent, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Copy } from "lucide-react";
import { command } from "../../api/commands";
import { Dialog } from "../../design/Dialog";
import { useAction } from "../access/useAction";
import { clientMessage } from "./model";

export type KeyAction =
  | { kind: "s3-create" }
  | { kind: "s3-delete"; key: string };
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
    "s3-create": "Create S3 credential",
    "s3-delete": "Revoke S3 credential",
  }[action.kind];
  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    if (deleting && confirmation !== clientId) return;
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
      } else if ("key" in action) {
        await command("credential.delete", { client_id: clientId, access_key_id: action.key });
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
              <Input
                readOnly
                value={item.value}
                autoComplete="off"
                spellCheck={false}
              />
              <Button
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
              </Button>
            </label>
          ))}
          {copyError && <p role="alert">{copyError}</p>}
          <label className="check-field">
            <Input
              type="checkbox"
              checked={saved}
              onChange={(e) => setSaved(e.target.checked)}
            />
            I have saved these keys. Secrets are shown only once.
          </label>
          <Button type="submit" variant="contained" className="primary" disabled={!saved} onClick={onClose}>
            Done
          </Button>
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
            {deleting && (
              <>
                <p className="full-field key-value">{action.key}</p>
                <label className="full-field">
                  Client ID to confirm
                  <Input
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
            <Button type="button" disabled={state.busy} onClick={onClose}>
              {state.unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button type="submit"
              className={deleting ? "danger" : "primary"}
              disabled={
                state.busy ||
                state.unknown ||
                (deleting && confirmation !== clientId)
              }
            >
              {state.busy ? "Saving..." : "Confirm"}
            </Button>
          </div>
        </form>
      )}
    </Dialog>
  );
}
