import {
  Alert,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Button,
  Stack,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { FormEvent, useId, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { command } from "../../api/commands";
import { message } from "../../api/http";
import { field } from "../../api/identity";
import { useAction } from "../access/useAction";
import { Storage, idPattern } from "../storages/model";
import { clientMessage, refreshClients } from "./model";

export function ClientDialog({
  id,
  onClose,
  onSaved,
}: {
  id?: string;
  onClose: () => void;
  onSaved: (id: string) => void;
}) {
  const state = useAction(clientMessage);
  const cache = useQueryClient();
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));
  const [confirmation, setConfirmation] = useState("");
  const storages = useQuery({
    queryKey: ["storages", "list"],
    enabled: !id,
    queryFn: ({ signal }) => command<Storage[]>("storage.list", {}, signal),
  });
  const options = storages.data?.filter((s) => s.kind === "s3") ?? [];
  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    if (id && confirmation !== id) return;
    const target = id ?? field(data, "id").trim();
    await state.run(async () => {
      if (id) await command("client.delete", { id });
      else
        await command("client.create", {
          id: target,
          storage_id: field(data, "storage_id"),
        });
      cache.removeQueries({ queryKey: ["clients", "detail", target] });
      await refreshClients(cache);
      onSaved(id ? "" : target);
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
        if (reason === "escapeKeyDown" && !state.busy) onClose();
      }}
    >
      <DialogTitle id={titleId}>
        {id ? "Delete client" : "Create client"}
      </DialogTitle>
      <DialogContent dividers>
        <form id={`${titleId}-form`} onSubmit={(e) => void submit(e)}>
          <Stack
            component="fieldset"
            spacing={3}
            sx={{ m: 0, p: 0, border: 0, minWidth: 0 }}
            disabled={state.busy || state.unknown}
          >
            {id ? (
              <>
                <Typography sx={{ overflowWrap: "anywhere" }}>
                  Delete <strong>{id}</strong> and its service keys. Referenced
                  files and pending cleanup block deletion.
                </Typography>
                <TextField
                  autoComplete="off"
                  value={confirmation}
                  onChange={(e) => setConfirmation(e.target.value)}
                  required
                  label={"Client ID to delete"}
                />
              </>
            ) : (
              <>
                <TextField
                  name="id"
                  required
                  autoComplete="off"
                  label={"Client ID"}
                  slotProps={{
                    htmlInput: { pattern: idPattern, maxLength: 64 },
                  }}
                />
                <TextField
                  name="storage_id"
                  required
                  defaultValue=""
                  disabled={storages.isError || storages.isPending}
                  label={"Storage"}
                  select
                  slotProps={{
                    htmlInput: { "aria-label": "Storage" },
                    select: { native: true },
                    inputLabel: { shrink: true },
                  }}
                >
                  <option value="" disabled>
                    Select storage
                  </option>
                  {options.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.id}
                    </option>
                  ))}
                </TextField>
                {storages.isPending && (
                  <Typography role="status">Loading storage...</Typography>
                )}
                {storages.isError && (
                  <Alert severity="error">
                    {message(storages.error)}{" "}
                    <Button
                      type="button"
                      onClick={() => void storages.refetch()}
                    >
                      Retry storage
                    </Button>
                  </Alert>
                )}
                {!storages.isPending &&
                  !storages.isError &&
                  !options.length && (
                    <Alert severity="info">No S3 storage registered.</Alert>
                  )}
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
          variant={id ? "outlined" : "contained"}
          color={id ? "error" : "primary"}
          disabled={
            state.busy ||
            state.unknown ||
            (id ? confirmation !== id : !options.length || storages.isError)
          }
        >
          {state.busy ? "Saving..." : id ? "Confirm delete" : "Create"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
