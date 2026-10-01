import {
  Alert,
  Divider,
  Typography,
  Stack,
  DialogContent,
  DialogActions,
  TextField,
  Button,
  Dialog,
  DialogTitle,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { FormEvent, useState, useId } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { identity, ApiError, message, request } from "../../api/http";
import { field } from "../../api/identity";

import {
  IssuedSetup,
  IssuedSetupLink,
  isIssuedSetup,
} from "./PasswordSetupIssue";

export function CreateUserDialog({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (id: string) => Promise<void>;
}) {
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));

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
      if (!isIssuedSetup(result))
        throw new Error("Invalid account creation response");
      setIssued(result);
    } catch (failure) {
      if (
        failure instanceof ApiError &&
        [400, 401, 403, 409, 429].includes(failure.status)
      ) {
        if (failure.status === 403)
          void cache.invalidateQueries({ queryKey: ["session"] });
        setError(
          failure.status === 401
            ? "Current password is incorrect or the session expired."
            : failure.status === 409
              ? "The username is already in use."
              : failure.status === 400
                ? "Check the name and username."
                : message(failure),
        );
      } else {
        setUnknown(true);
        setError(
          "The result is unknown. Close and review accounts before trying again.",
        );
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog
      open
      fullWidth
      maxWidth={issued ? "sm" : "xs"}
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !pending && !issued) onClose();
      }}
    >
      <DialogTitle id={titleId}>
        {issued ? "Save setup link" : "Create account"}
      </DialogTitle>
      {issued ? (
        <IssuedSetupLink
          issued={issued}
          onDone={() => {
            onClose();
            void onCreated(issued.account_id);
          }}
        />
      ) : (
        <>
          <DialogContent dividers>
            <form
              id={`${titleId}-form`}
              onSubmit={(event) => {
                void submit(event);
              }}
            >
              <Stack spacing={3}>
                <Typography component="h3" variant="subtitle1">Account details</Typography>
                <TextField
                  name="display_name"
                  required
                  label={"Display name"}
                  disabled={pending || unknown}
                  slotProps={{ htmlInput: { maxLength: 80 } }}
                />
                <TextField
                  name="username"
                  disabled={pending || unknown}
                  autoComplete="off"
                  required
                  label={"Username"}
                  slotProps={{
                    htmlInput: {
                      minLength: 3,
                      maxLength: 64,
                      pattern: "[A-Za-z0-9][A-Za-z0-9._-]{2,63}",
                    },
                  }}
                />
                <TextField
                  name="role"
                  disabled={pending || unknown}
                  defaultValue="reader"
                  label={"Role"}
                  select
                  slotProps={{
                    htmlInput: { "aria-label": "Role" },
                    select: { native: true },
                  }}
                >
                  <option value="reader">Reader</option>
                  <option value="writer">Writer</option>
                  <option value="admin">Admin</option>
                </TextField>
                <Divider />
                <Typography component="h3" variant="subtitle1">Confirm your identity</Typography>
                <TextField
                  name="current_password"
                  type="password"
                  autoComplete="current-password"
                  required
                  label={"Your current password"}
                  disabled={pending || unknown}
                />
                {error && <Alert severity="error">{error}</Alert>}
              </Stack>
            </form>
          </DialogContent>
          <DialogActions>
            <Button type="button" disabled={pending} onClick={onClose}>
              {unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button
              type="submit"
              form={`${titleId}-form`}
              variant="contained"
              disabled={pending || unknown}
            >
              {pending ? "Creating..." : "Create account"}
            </Button>
          </DialogActions>
        </>
      )}
    </Dialog>
  );
}
