import { Input, Select } from "../../design/Fields";
import { Button } from "@mui/material";
import { FormEvent, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { identity, ApiError, message, request } from "../../api/http";
import { field } from "../../api/identity";
import { Dialog } from "../../design/Dialog";
import { IssuedSetup, IssuedSetupLink, isIssuedSetup } from "./PasswordSetupIssue";

export function CreateUserDialog({ onClose, onCreated }: {
  onClose: () => void;
  onCreated: (id: string) => Promise<void>;
}) {
  const cache = useQueryClient();
  const [pending, setPending] = useState(false);
  const [unknown, setUnknown] = useState(false);
  const [error, setError] = useState("");
  const [issued, setIssued] = useState<IssuedSetup | null>(null);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending || unknown) return;
    const form = event.currentTarget;
    const data = new FormData(form);
    form.reset();
    setPending(true);
    setError("");
    try {
      const result = await request<unknown>(`${identity}/accounts`, {
        method: "POST",
        body: JSON.stringify({
          kind: "user_with_password_setup",
          display_name: field(data, "display_name").trim(),
          role: field(data, "role"),
          username: field(data, "username").trim(),
          current_password: field(data, "current_password"),
        }),
      });
      if (!isIssuedSetup(result)) throw new Error("Invalid account creation response");
      setIssued(result);
    } catch (failure) {
      if (failure instanceof ApiError && [400, 401, 403, 409, 429].includes(failure.status)) {
        if (failure.status === 403) void cache.invalidateQueries({ queryKey: ["session"] });
        setError(failure.status === 401 ? "Current password is incorrect or the session expired."
          : failure.status === 409 ? "The username is already in use."
            : failure.status === 400 ? "Check the name and username."
              : message(failure));
      } else {
        setUnknown(true);
        setError("The result is unknown. Close and review accounts before trying again.");
      }
    } finally {
      setPending(false);
    }
  }

  return <Dialog title={issued ? "Save setup link" : "Create user"} busy={pending} closeDisabled={Boolean(issued)} onClose={onClose}>
    {issued ? <IssuedSetupLink issued={issued} onDone={() => {
      onClose();
      void onCreated(issued.account_id);
    }} /> : <form onSubmit={(event) => { void submit(event); }}>
      <fieldset className="storage-form" disabled={pending || unknown}>
        <label className="full-field">Name<Input name="display_name" required maxLength={80} /></label>
        <label>Username<Input name="username" autoComplete="off" required minLength={3} maxLength={64} pattern="[A-Za-z0-9][A-Za-z0-9._-]{2,63}" /></label>
        <label>Role<Select name="role" aria-label="Role" defaultValue="reader">
          <option value="reader">Reader</option><option value="writer">Writer</option><option value="admin">Admin</option>
        </Select></label>
        <label className="full-field">Your current password<Input name="current_password" type="password" autoComplete="current-password" required /></label>
      </fieldset>
      {error && <p role="alert" className="form-error">{error}</p>}
      <div className="dialog-actions">
        <Button type="button" disabled={pending} onClick={onClose}>{unknown ? "Close and review" : "Cancel"}</Button>
        <Button type="submit" variant="contained" className="primary" disabled={pending || unknown}>{pending ? "Creating..." : "Create user"}</Button>
      </div>
    </form>}
  </Dialog>;
}
