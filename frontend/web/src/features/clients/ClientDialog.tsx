import { Input, Select } from "../../design/Fields";
import { Button } from "@mui/material";
import { FormEvent, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Dialog } from "../../design/Dialog";
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
      title={id ? "Delete client" : "Create client"}
      busy={state.busy}
      onClose={onClose}
    >
      <form onSubmit={(e) => void submit(e)}>
        <fieldset
          className="storage-form"
          disabled={state.busy || state.unknown}
        >
          {id ? (
            <>
              <p className="full-field">
                Delete <strong>{id}</strong> and its service keys. Referenced
                files and pending cleanup block deletion.
              </p>
              <label className="full-field">
                Client ID to delete
                <Input
                  autoComplete="off"
                  value={confirmation}
                  onChange={(e) => setConfirmation(e.target.value)}
                  required
                />
              </label>
            </>
          ) : (
            <>
              <label>
                Client ID
                <Input
                  name="id"
                  pattern={idPattern}
                  maxLength={64}
                  required
                  autoComplete="off"
                />
              </label>
              <label>
                Storage
                <Select
                  name="storage_id"
                  aria-label="Storage"
                  required
                  defaultValue=""
                  disabled={storages.isError || storages.isPending}
                >
                  <option value="" disabled>
                    Select storage
                  </option>
                  {options.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.id}
                    </option>
                  ))}
                </Select>
              </label>
              {storages.isPending && <p role="status">Loading storage...</p>}
              {storages.isError && (
                <p role="alert">
                  {message(storages.error)}{" "}
                  <Button type="button" onClick={() => void storages.refetch()}>
                    Retry storage
                  </Button>
                </p>
              )}
              {!storages.isPending && !storages.isError && !options.length && (
                <p>No S3 storage registered.</p>
              )}
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
            variant={id ? "outlined" : "contained"}
            color={id ? "error" : "primary"}
            className={id ? "danger" : "primary"}
            disabled={
              state.busy ||
              state.unknown ||
              (id ? confirmation !== id : !options.length || storages.isError)
            }
          >
            {state.busy ? "Saving..." : id ? "Confirm delete" : "Create"}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
