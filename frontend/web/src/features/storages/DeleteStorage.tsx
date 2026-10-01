import {
  Alert,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Button,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { FormEvent, useId, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { ApiError } from "../../api/http";
import { command } from "../../api/commands";
import { clearSession } from "../../auth/session";
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
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));
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
    <Dialog
      open
      fullWidth
      maxWidth="sm"
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !busy) onClose();
      }}
    >
      <DialogTitle id={titleId}>Delete storage</DialogTitle>
      <DialogContent dividers>
        <form id={`${titleId}-form`} onSubmit={(event) => void remove(event)}>
          <Typography sx={{ mb: 3, overflowWrap: "anywhere" }}>
            Remove <strong>{id}</strong> from the registry. The S3 bucket is
            retained.
          </Typography>
          <TextField
            value={confirmation}
            onChange={(event) => setConfirmation(event.target.value)}
            disabled={busy || unknown}
            autoComplete="off"
            label={"Storage ID to delete"}
          />
          {error && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {error}
            </Alert>
          )}
        </form>
      </DialogContent>
      <DialogActions>
        <Button
          type="button"
          onClick={unknown ? onReturnToList : onClose}
          disabled={busy}
        >
          {unknown ? "Review list" : "Cancel"}
        </Button>
        <Button
          color="error"
          variant="contained"
          type="submit"
          form={`${titleId}-form`}
          startIcon={<Trash2 size={16} />}
          disabled={busy || unknown || confirmation !== id}
        >
          {busy ? "Deleting..." : "Confirm delete"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
