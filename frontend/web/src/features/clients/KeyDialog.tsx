import {
  DialogContent,
  Dialog,
  DialogTitle,
  DialogActions,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
  Stack,
  IconButton,
  Tooltip,
  Alert,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { FormEvent, useId, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Copy } from "lucide-react";
import { command } from "../../api/commands";
import { useAction } from "../access/useAction";
import { clientMessage } from "./model";

export type KeyAction =
  { kind: "s3-create" } | { kind: "s3-delete"; key: string };
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
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));
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
      open
      fullWidth
      maxWidth="sm"
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !state.busy && (!issued || saved))
          onClose();
      }}
    >
      <DialogTitle id={titleId}>{title}</DialogTitle>
      {issued ? (
        <>
          <DialogContent dividers>
            <Stack spacing={3}>
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
                      input: {
                        readOnly: true,
                        sx: { fontFamily: "monospace" },
                      },
                    }}
                  />
                  <Tooltip title={`Copy ${item.name}`}>
                    <IconButton
                      type="button"
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
              {copyError && <Alert severity="error">{copyError}</Alert>}
            </Stack>
          </DialogContent>
          <DialogActions
            sx={{ flexDirection: "column", alignItems: "stretch", gap: 1 }}
          >
            <FormControlLabel
              control={
                <Checkbox
                  checked={saved}
                  onChange={(e) => setSaved(e.target.checked)}
                />
              }
              label={"I have saved these keys. Secrets are shown only once."}
            />
            <Button
              type="button"
              variant="contained"
              sx={{ alignSelf: "flex-end" }}
              disabled={!saved}
              onClick={onClose}
            >
              Done
            </Button>
          </DialogActions>
        </>
      ) : (
        <>
          <DialogContent dividers>
            <form id={`${titleId}-form`} onSubmit={(e) => void submit(e)}>
              <Stack
                component="fieldset"
                spacing={3}
                sx={{ m: 0, p: 0, border: 0, minWidth: 0 }}
                disabled={state.busy || state.unknown}
              >
                <Typography sx={{ overflowWrap: "anywhere" }}>
                  Client: <strong>{clientId}</strong>
                </Typography>
                {deleting && (
                  <>
                    <Typography
                      component="code"
                      variant="body2"
                      sx={{ fontFamily: "monospace", overflowWrap: "anywhere" }}
                    >
                      {action.key}
                    </Typography>
                    <TextField
                      autoComplete="off"
                      required
                      value={confirmation}
                      onChange={(e) => setConfirmation(e.target.value)}
                      label={"Client ID to confirm"}
                    />
                  </>
                )}
              </Stack>
              {state.error && (
                <Alert severity="error" sx={{ mt: 2 }}>
                  {state.error}
                </Alert>
              )}
            </form>
          </DialogContent>
          <DialogActions>
            <Button type="button" disabled={state.busy} onClick={onClose}>
              {state.unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button
              type="submit"
              form={`${titleId}-form`}
              variant="contained"
              color={deleting ? "error" : "primary"}
              disabled={
                state.busy ||
                state.unknown ||
                (deleting && confirmation !== clientId)
              }
            >
              {state.busy ? "Saving..." : "Confirm"}
            </Button>
          </DialogActions>
        </>
      )}
    </Dialog>
  );
}
