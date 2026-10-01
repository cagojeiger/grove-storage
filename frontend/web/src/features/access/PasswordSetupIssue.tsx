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
import { Copy, Link2 } from "lucide-react";
import { identity, ApiError, message, request } from "../../api/http";
import { field, Account } from "../../api/identity";

export type IssuedSetup = {
  account_id: string;
  username: string;
  expires_at: string;
  token: string;
};
export function isIssuedSetup(
  value: unknown,
  account?: string,
): value is IssuedSetup {
  if (!value || typeof value !== "object") return false;
  const result = value as Record<string, unknown>;
  return (
    typeof result.account_id === "string" &&
    (!account || result.account_id === account) &&
    typeof result.username === "string" &&
    typeof result.expires_at === "string" &&
    Number.isFinite(Date.parse(result.expires_at)) &&
    typeof result.token === "string" &&
    /^gsps_[0-9a-f]{64}$/.test(result.token)
  );
}

export function PasswordSetupIssue({ account }: { account: Account }) {
  const [open, setOpen] = useState(false);
  return (
    <Stack component="section" aria-label="Password setup" spacing={2}>
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={2}
        sx={{ alignItems: { sm: "center" }, justifyContent: "space-between" }}
      >
        <Typography component="h2" variant="h6">
          Password setup
        </Typography>
        <Button
          startIcon={<Link2 size={16} />}
          disabled={!account.is_active || Boolean(account.deleted_at)}
          onClick={() => setOpen(true)}
        >
          Issue setup link
        </Button>
      </Stack>
      {open && <SetupDialog account={account} onClose={() => setOpen(false)} />}
    </Stack>
  );
}

export function IssuedSetupLink({
  issued,
  onDone,
}: {
  issued: IssuedSetup;
  onDone: () => void;
}) {
  const [acknowledged, setAcknowledged] = useState(false);
  const [copyError, setCopyError] = useState("");
  const link = new URL(
    `${import.meta.env.BASE_URL}#set-password/${issued.token}`,
    window.location.origin,
  ).toString();
  return (
    <>
      <DialogContent dividers>
        <Stack spacing={3}>
          <Typography>
            Share this link privately with {issued.username}. It is shown once
            and expires in 24 hours.
          </Typography>
          <TextField
            value={link}
            label="Setup link"
            slotProps={{
              htmlInput: {
                "aria-label": "Setup link",
                onFocus: (event: React.FocusEvent<HTMLInputElement>) =>
                  event.currentTarget.select(),
              },
              input: { readOnly: true },
            }}
          />
          <Button
            type="button"
            startIcon={<Copy size={16} />}
            onClick={() => {
              if (!navigator.clipboard) {
                setCopyError("Copy unavailable. Select the link manually.");
                return;
              }
              void navigator.clipboard
                .writeText(link)
                .catch(() =>
                  setCopyError("Copy failed. Select the link manually."),
                );
            }}
          >
            Copy link
          </Button>
          {copyError && <Alert severity="error">{copyError}</Alert>}
          <FormControlLabel
            control={
              <Checkbox
                checked={acknowledged}
                onChange={(event) => setAcknowledged(event.target.checked)}
              />
            }
            label={"I have saved this setup link."}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button variant="contained" disabled={!acknowledged} onClick={onDone}>
          Done
        </Button>
      </DialogActions>
    </>
  );
}

function SetupDialog({
  account,
  onClose,
}: {
  account: Account;
  onClose: () => void;
}) {
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));

  const [pending, setPending] = useState(false);
  const [unknown, setUnknown] = useState(false);
  const [error, setError] = useState("");
  const [issued, setIssued] = useState<IssuedSetup | null>(null);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    form.reset();
    if (pending || unknown) return;
    setPending(true);
    setError("");
    try {
      const result = await request<unknown>(
        `${identity}/accounts/${encodeURIComponent(account.id)}/password-setup`,
        {
          method: "POST",
          body: JSON.stringify({
            username: field(data, "username").trim(),
            current_password: field(data, "current_password"),
          }),
        },
      );
      if (!isIssuedSetup(result, account.id))
        throw new Error("Invalid setup response");
      setIssued(result);
    } catch (failure) {
      if (
        failure instanceof ApiError &&
        [400, 401, 403, 404, 409, 429].includes(failure.status)
      ) {
        setError(
          failure.status === 401
            ? "Current password is incorrect or the session expired."
            : failure.status === 409
              ? "The username is in use or this account already has a password."
              : failure.status === 400
                ? "Enter a valid username."
                : message(failure),
        );
      } else {
        setUnknown(true);
        setError(
          "The result is unknown. Close and refresh this account before issuing another link.",
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
      maxWidth="sm"
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !pending && !issued) onClose();
      }}
    >
      <DialogTitle id={titleId}>
        {issued ? "Save setup link" : "Issue setup link"}
      </DialogTitle>
      {issued ? (
        <IssuedSetupLink issued={issued} onDone={onClose} />
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
                  name="current_password"
                  disabled={pending || unknown}
                  type="password"
                  autoComplete="current-password"
                  required
                  label={"Current password"}
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
              {pending ? "Issuing..." : "Issue link"}
            </Button>
          </DialogActions>
        </>
      )}
    </Dialog>
  );
}
