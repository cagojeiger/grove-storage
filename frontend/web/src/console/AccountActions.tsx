import { useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Alert,
  Checkbox,
  Divider,
  FormControlLabel,
  Typography,
} from "@mui/material";
import {
  type Account,
  field,
  identityRequest,
  isChanged,
} from "../api/identity";
import { identity, request } from "../api/http";
import { useAction } from "../hooks/useAction";
import { FormDialog, SecretDialog } from "./ui";
import {
  isSetupLink,
  reauthenticationError,
  type SetupLink,
} from "./identityModel";

export type AccountAction = "name" | "role" | "active" | "delete";
export function AccountEdit({
  account,
  mode,
  self,
  onClose,
  onSaved,
}: {
  account: Account;
  mode: AccountAction;
  self: boolean;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const action = useAction();
  const [name, setName] = useState(account.display_name);
  const [role, setRole] = useState(account.role);
  const [confirmation, setConfirmation] = useState("");
  const [ack, setAck] = useState(false);
  const danger = mode === "delete" || (mode === "active" && account.is_active);
  const selfImpact =
    self && (danger || (mode === "role" && role !== account.role));
  const title =
    mode === "name"
      ? "Edit name"
      : mode === "role"
        ? "Change role"
        : mode === "delete"
          ? "Delete account"
          : account.is_active
            ? "Disable account"
            : "Enable account";
  const invalid =
    (mode === "name" &&
      (!name.trim() ||
        name.trim() === account.display_name ||
        Array.from(name.trim()).length > 80)) ||
    (danger && confirmation !== account.display_name) ||
    (selfImpact && !ack);
  return (
    <FormDialog
      title={title}
      action={action}
      onClose={onClose}
      danger={danger}
      disabled={invalid}
      submit={async () => {
        if (invalid) return;
        await identityRequest(
          `/accounts/${encodeURIComponent(account.id)}`,
          isChanged,
          {
            method: mode === "delete" ? "DELETE" : "PATCH",
            ...(mode !== "delete"
              ? {
                  body: JSON.stringify(
                    mode === "name"
                      ? { operation: "name", display_name: name.trim() }
                      : mode === "role"
                        ? { operation: "role", role }
                        : {
                            operation: "active",
                            is_active: !account.is_active,
                          },
                  ),
                }
              : {}),
          },
        );
        await onSaved();
        onClose();
      }}
    >
      {mode === "name" ? (
        <TextField
          fullWidth
          label="Display name"
          required
          value={name}
          onChange={(event) => setName(event.target.value)}
          slotProps={{ htmlInput: { maxLength: 80 } }}
        />
      ) : (
        <Typography>{account.display_name}</Typography>
      )}
      {mode === "role" && (
        <>
          <TextField
            fullWidth
            select
            slotProps={{ select: { native: true } }}
            label="Role"
            value={role}
            onChange={(event) => {
              setRole(event.target.value as Account["role"]);
              setAck(false);
            }}
          >
            {["reader", "writer", "admin"].map((value) => (
              <option key={value} value={value}>
                {value.charAt(0).toUpperCase() + value.slice(1)}
              </option>
            ))}
          </TextField>
          <Typography variant="body2">
            This applies to existing tokens and sessions.
          </Typography>
        </>
      )}
      {danger && (
        <>
          <Alert severity="warning" role="note">
            All account sessions will be revoked. Storage, clients and files are
            preserved.
          </Alert>
          <TextField
            fullWidth
            label="Account name to confirm"
            helperText={`Enter ${account.display_name}`}
            value={confirmation}
            onChange={(event) => setConfirmation(event.target.value)}
            required
          />
        </>
      )}
      {mode === "active" && !account.is_active && (
        <Typography variant="body2">
          Unexpired, unrevoked tokens become usable again. Previous sessions
          remain revoked.
        </Typography>
      )}
      {selfImpact && (
        <FormControlLabel
          control={
            <Checkbox
              checked={ack}
              onChange={(event) => setAck(event.target.checked)}
            />
          }
          label="I understand this changes my access."
        />
      )}
    </FormDialog>
  );
}
export function AccountSetup({
  account,
  onClose,
  onSaved,
}: {
  account?: Account;
  onClose: () => void;
  onSaved: (id: string) => Promise<void>;
}) {
  const action = useAction(reauthenticationError, true);
  const [issued, setIssued] = useState<SetupLink | null>(null);
  if (issued)
    return (
      <SecretDialog
        title="Save setup link"
        values={{
          "Setup link": new URL(
            `${import.meta.env.BASE_URL}#set-password/${issued.token}`,
            location.origin,
          ).toString(),
        }}
        acknowledgement="I have saved this setup link."
        details={
          <Typography variant="body2">
            For {issued.username}. Expires{" "}
            {new Date(issued.expires_at).toLocaleString("en-US")}.
          </Typography>
        }
        onClose={() => {
          void onSaved(issued.account_id);
          onClose();
        }}
      />
    );
  return (
    <FormDialog
      title={account ? "Issue setup link" : "Create account"}
      action={action}
      onClose={onClose}
      button={account ? "Issue link" : "Create account"}
      submit={async (data, form) => {
        const body = {
          username: field(data, "username").trim().toLowerCase(),
          current_password: field(data, "current_password"),
          ...(!account
            ? {
                kind: "user_with_password_setup",
                display_name: field(data, "display_name").trim(),
                role: field(data, "role"),
              }
            : {}),
        };
        const password = form.elements.namedItem("current_password");
        if (password instanceof HTMLInputElement) password.value = "";
        const value = await request<unknown>(
          `${identity}/accounts${account ? `/${encodeURIComponent(account.id)}/password-setup` : ""}`,
          { method: "POST", body: JSON.stringify(body) },
        );
        if (!isSetupLink(value) || (account && value.account_id !== account.id))
          throw new Error("Invalid setup response");
        setIssued(value);
      }}
    >
      {!account && (
        <Typography component="h3" variant="subtitle2">New account</Typography>
      )}
      {!account && (
        <TextField
          fullWidth
          label="Display name"
          name="display_name"
          required
          slotProps={{ htmlInput: { maxLength: 80 } }}
        />
      )}
      <TextField
        fullWidth
        label="Username"
        name="username"
        defaultValue={account?.username ?? ""}
        required
        autoComplete="off"
        slotProps={{
          input: { readOnly: Boolean(account?.username) },
          htmlInput: {
            pattern: "[A-Za-z0-9][A-Za-z0-9._\\-]{2,63}",
            minLength: 3,
            maxLength: 64,
          },
        }}
      />
      {!account && (
        <TextField
          fullWidth
          label="Role"
          name="role"
          select
          slotProps={{ select: { native: true } }}
          defaultValue="reader"
        >
          {["reader", "writer", "admin"].map((role) => (
            <option key={role} value={role}>
              {role.charAt(0).toUpperCase() + role.slice(1)}
            </option>
          ))}
        </TextField>
      )}
      {!account && (
        <Divider textAlign="left">
          <Typography component="h3" variant="subtitle2">Administrator verification</Typography>
        </Divider>
      )}
      <TextField
        fullWidth
        label={account ? "Current password" : "Administrator password"}
        name="current_password"
        type="password"
        autoComplete="current-password"
        required
      />
    </FormDialog>
  );
}
