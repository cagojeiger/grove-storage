import { Alert, Stack, Typography, TextField, Button } from "@mui/material";

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
  const [confirmationError, setConfirmationError] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const form = event.currentTarget;
    const fields = new FormData(form);
    const currentPassword = field(fields, "current_password");
    const newPassword = field(fields, "new_password");
    const confirmation = field(fields, "confirmation");
    if (newPassword !== confirmation) {
      setConfirmationError("The new passwords do not match.");
      (form.elements.namedItem("confirmation") as HTMLInputElement).focus();
      return;
    }
    form.reset();
    setConfirmationError("");
    setPending(true);
    setError("");
    try {
      await request(`${identity}/me/password`, {
        method: "POST",
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      });
      clearSession(cache);
    } catch (failure) {
      setError(
        failure instanceof ApiError && failure.status === 401
          ? "The current password is incorrect or this session has expired."
          : failure instanceof ApiError && failure.status === 400
            ? "Choose a different password of 15 to 128 characters."
            : message(failure),
      );
    } finally {
      setPending(false);
    }
  }

  return (
    <Stack component="section" spacing={3} aria-labelledby="password-heading">
      <Typography component="h2" variant="h6" id="password-heading">
        Password
      </Typography>
      <Stack
        component="form"
        spacing={3}
        onSubmit={(event) => {
          void submit(event);
        }}
      >
        <TextField
          label="Current password"
          id="current-password"
          name="current_password"
          type="password"
          autoComplete="current-password"
          required
          disabled={pending}
        />
        <TextField
          label="New password"
          id="new-password"
          name="new_password"
          type="password"
          autoComplete="new-password"
          required
          disabled={pending}
          slotProps={{ htmlInput: { minLength: 15, maxLength: 128 } }}
        />
        <TextField
          label="Confirm new password"
          id="confirm-password"
          name="confirmation"
          type="password"
          autoComplete="new-password"
          required
          disabled={pending}
          error={Boolean(confirmationError)}
          helperText={confirmationError}
          onChange={() => setConfirmationError("")}
          slotProps={{
            htmlInput: { minLength: 15, maxLength: 128 },
            formHelperText: { role: confirmationError ? "alert" : undefined },
          }}
        />
        {error && <Alert severity="error">{error}</Alert>}
        <Button
          sx={{ alignSelf: { sm: "flex-start" } }}
          variant="contained"
          type="submit"
          disabled={pending}
          startIcon={<KeyRound size={16} />}
        >
          {pending ? "Changing password" : "Change password"}
        </Button>
      </Stack>
    </Stack>
  );
}
