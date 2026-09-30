import {
  DialogContent,
  DialogActions,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
  Stack,
  IconButton,
  Tooltip,
} from "@mui/material";

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
        await command("credential.delete", {
          client_id: clientId,
          access_key_id: action.key,
        });
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
        <DialogContent>
          <div className="issued-token">
            {issued.map((item) => (
              <Stack
                key={item.name}
                direction="row"
                spacing={1}
                sx={{ alignItems: "center" }}
              >
                <TextField
                  label={item.name}
                  value={item.value}
                  autoComplete="off"
                  slotProps={{
                    htmlInput: { spellCheck: false },
                    input: { readOnly: true },
                  }}
                />
                <Tooltip title={`Copy ${item.name}`}>
                  <IconButton
                    type="button"
                    className="icon-button"
                    title={`Copy ${item.name}`}
                    aria-label={`Copy ${item.name}`}
                    onClick={() => {
                      setCopyError("");
                      void navigator.clipboard
                        .writeText(item.value)
                        .catch(() =>
                          setCopyError(
                            "Copy failed. Select the key to copy it.",
                          ),
                        );
                    }}
                  >
                    <Copy size={16} />
                  </IconButton>
                </Tooltip>
              </Stack>
            ))}
            {copyError && <p role="alert">{copyError}</p>}
            <FormControlLabel
              className="check-field"
              control={
                <Checkbox
                  checked={saved}
                  onChange={(e) => setSaved(e.target.checked)}
                />
              }
              label={"I have saved these keys. Secrets are shown only once."}
            />
            <Button
              type="submit"
              variant="contained"
              className="primary"
              disabled={!saved}
              onClick={onClose}
            >
              Done
            </Button>
          </div>
        </DialogContent>
      ) : (
        <form onSubmit={(e) => void submit(e)}>
          <DialogContent>
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
                  <TextField
                    autoComplete="off"
                    required
                    value={confirmation}
                    onChange={(e) => setConfirmation(e.target.value)}
                    label={"Client ID to confirm"}
                    className="full-field"
                  />
                </>
              )}
            </fieldset>
            {state.error && (
              <p className="form-error" role="alert">
                {state.error}
              </p>
            )}
          </DialogContent>
          <DialogActions>
            <Button type="button" disabled={state.busy} onClick={onClose}>
              {state.unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button
              type="submit"
              variant="contained"
              color={deleting ? "error" : "primary"}
              className={deleting ? "danger" : "primary"}
              disabled={
                state.busy ||
                state.unknown ||
                (deleting && confirmation !== clientId)
              }
            >
              {state.busy ? "Saving..." : "Confirm"}
            </Button>
          </DialogActions>
        </form>
      )}
    </Dialog>
  );
}
