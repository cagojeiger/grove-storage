import { Button } from "@mui/material";
import { Input } from "../../design/Fields";
import { FormEvent, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { KeyRound } from "lucide-react";
import { ApiError, identity, message, request } from "../../api/http";
import { clearSession } from "../../auth/session";
import { field } from "../../api/identity";

export function PasswordChange() {
  const cache = useQueryClient();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const form = event.currentTarget;
    const fields = new FormData(form);
    const currentPassword = field(fields, "current_password");
    const newPassword = field(fields, "new_password");
    const confirmation = field(fields, "confirmation");
    if (newPassword !== confirmation) {
      setError("The new passwords do not match.");
      return;
    }
    form.reset();
    setPending(true);
    setError("");
    try {
      await request(`${identity}/me/password`, {
        method: "POST",
        body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
      });
      clearSession(cache);
    } catch (failure) {
      setError(failure instanceof ApiError && failure.status === 401
        ? "The current password is incorrect or this session has expired."
        : failure instanceof ApiError && failure.status === 400
          ? "Choose a different password of 15 to 128 characters."
          : message(failure));
    } finally {
      setPending(false);
    }
  }

  return (
    <section className="storage-section" aria-labelledby="password-heading">
      <h2 id="password-heading">Password</h2>
      <form className="account-password-form" onSubmit={(event) => { void submit(event); }}>
        <label htmlFor="current-password">Current password</label>
        <Input id="current-password" name="current_password" type="password" autoComplete="current-password" required disabled={pending} />
        <label htmlFor="new-password">New password</label>
        <Input id="new-password" name="new_password" type="password" autoComplete="new-password" minLength={15} maxLength={128} required disabled={pending} />
        <label htmlFor="confirm-password">Confirm new password</label>
        <Input id="confirm-password" name="confirmation" type="password" autoComplete="new-password" minLength={15} maxLength={128} required disabled={pending} />
        {error && <p role="alert">{error}</p>}
        <Button variant="contained" type="submit" disabled={pending}>
          <KeyRound size={16} aria-hidden="true" />
          {pending ? "Changing password" : "Change password"}
        </Button>
      </form>
    </section>
  );
}
