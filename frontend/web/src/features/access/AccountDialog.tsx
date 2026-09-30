import {
  DialogContent,
  DialogActions,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
  Alert,
  Stack,
  Typography,
  Dialog,
  DialogTitle,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { FormEvent, useState, useId } from "react";
import { Account, identityRequest, isChanged } from "../../api/identity";

import { useAction } from "./useAction";

export type AccountAction = "name" | "role" | "active" | "delete";
export function AccountDialog({
  account,
  action,
  isSelf = false,
  onClose,
  onSaved,
}: {
  account?: Account;
  action: AccountAction;
  isSelf?: boolean;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));

  const state = useAction();
  const [confirmation, setConfirmation] = useState("");
  const [name, setName] = useState(
    action === "name" ? (account?.display_name ?? "") : "",
  );
  const [role, setRole] = useState(account?.role ?? "reader");
  const [acknowledged, setAcknowledged] = useState(false);
  const title =
    action === "name"
      ? "Edit name"
      : action === "role"
        ? "Change role"
        : action === "delete"
          ? "Delete account"
          : account?.is_active
            ? "Disable account"
            : "Enable account";
  const dangerous =
    action === "delete" || (action === "active" && account?.is_active);
  const selfDemotion =
    isSelf &&
    action === "role" &&
    account?.role === "admin" &&
    role !== "admin";
  const selfImpact = isSelf && (dangerous || selfDemotion);
  const invalidName =
    action === "name" &&
    (!name.trim() ||
      Array.from(name.trim()).length > 80 ||
      name.trim() === account?.display_name);
  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    if (dangerous && confirmation !== account?.display_name) return;
    if (invalidName || (selfImpact && !acknowledged)) return;
    await state.run(async () => {
      if (account) {
        await identityRequest(
          `/accounts/${encodeURIComponent(account.id)}`,
          isChanged,
          {
            method: action === "delete" ? "DELETE" : "PATCH",
            ...(action === "delete"
              ? {}
              : {
                  body: JSON.stringify(
                    action === "name"
                      ? { operation: "name", display_name: name.trim() }
                      : action === "role"
                        ? { operation: "role", role }
                        : {
                            operation: "active",
                            is_active: !account.is_active,
                          },
                  ),
                }),
          },
        );
      }
      onClose();
      await onSaved();
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
      <DialogTitle id={titleId}>{title}</DialogTitle>
      <>
        <DialogContent dividers>
          <form id={`${titleId}-form`} onSubmit={(e) => void submit(e)}>
            <Stack spacing={3}>
              {action === "name" ? (
                <TextField
                  name="display_name"
                  required
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  label={"Name"}
                  disabled={state.busy || state.unknown}
                  slotProps={{ htmlInput: { maxLength: 80 } }}
                />
              ) : (
                <Typography sx={{ overflowWrap: "anywhere" }}>
                  {account?.display_name}
                </Typography>
              )}
              {action === "role" && (
                <TextField
                  name="role"
                  value={role}
                  onChange={(e) => {
                    setRole(e.target.value as Account["role"]);
                    setAcknowledged(false);
                  }}
                  label={"Role"}
                  disabled={state.busy || state.unknown}
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
              )}
              {action === "delete" && (
                <Typography color="error">
                  All tokens and sessions belonging to this User will be
                  revoked. Storage, clients, and files are preserved.
                </Typography>
              )}
              {action === "role" && (
                <Typography>
                  The selected role applies to existing tokens and sessions.
                </Typography>
              )}
              {action === "active" && !account?.is_active && (
                <Typography>
                  Unexpired, unrevoked tokens become usable again. Previous
                  sessions remain revoked.
                </Typography>
              )}
              {selfImpact && (
                <FormControlLabel
                  disabled={state.busy || state.unknown}
                  control={
                    <Checkbox
                      checked={acknowledged}
                      onChange={(e) => setAcknowledged(e.target.checked)}
                    />
                  }
                  label={
                    selfDemotion
                      ? "I understand I will lose access to Accounts."
                      : "I understand my current session will end."
                  }
                />
              )}
              {action === "active" && account?.is_active && (
                <Typography color="error">
                  Access is suspended for all of this User's tokens. Existing
                  sessions are revoked.
                </Typography>
              )}
              {dangerous && (
                <TextField
                  value={confirmation}
                  onChange={(e) => setConfirmation(e.target.value)}
                  autoComplete="off"
                  required
                  label={"Confirm account name"}
                  disabled={state.busy || state.unknown}
                />
              )}
              {state.error && <Alert severity="error">{state.error}</Alert>}
            </Stack>
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
            color={dangerous ? "error" : "primary"}
            disabled={
              state.busy ||
              state.unknown ||
              invalidName ||
              (selfImpact && !acknowledged) ||
              Boolean(dangerous && confirmation !== account?.display_name)
            }
          >
            {state.busy ? "Saving..." : action === "name" ? "Save name" : title}
          </Button>
        </DialogActions>
      </>
    </Dialog>
  );
}
