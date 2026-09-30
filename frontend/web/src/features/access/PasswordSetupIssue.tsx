import {
  DialogContent,
  DialogActions,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
} from "@mui/material";

import { FormEvent, useState } from "react";
import { Copy, Link2 } from "lucide-react";
import { identity, ApiError, message, request } from "../../api/http";
import { field, Account } from "../../api/identity";
import { Dialog } from "../../design/Dialog";

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
    <section className="storage-section" aria-label="Password setup">
      <div className="section-heading">
        <h2>Password setup</h2>
        <Button
          type="submit"
          startIcon={<Link2 size={16} />}
          disabled={!account.is_active || Boolean(account.deleted_at)}
          onClick={() => setOpen(true)}
        >
          Issue setup link
        </Button>
      </div>
      {open && <SetupDialog account={account} onClose={() => setOpen(false)} />}
    </section>
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
    <div className="storage-form">
      <p>
        Share this link privately with {issued.username}. It is shown once and
        expires in 24 hours.
      </p>
      <TextField
        value={link}
        label="Setup link"
        className="full-field"
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
      {copyError && <p role="alert">{copyError}</p>}
      <FormControlLabel
        className="check-field full-field"
        control={
          <Checkbox
            checked={acknowledged}
            onChange={(event) => setAcknowledged(event.target.checked)}
          />
        }
        label={"I have saved this setup link."}
      />
      <div className="dialog-actions">
        <Button
          type="submit"
          variant="contained"
          className="primary"
          disabled={!acknowledged}
          onClick={onDone}
        >
          Done
        </Button>
      </div>
    </div>
  );
}

function SetupDialog({
  account,
  onClose,
}: {
  account: Account;
  onClose: () => void;
}) {
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
      title={issued ? "Save setup link" : "Issue setup link"}
      busy={pending}
      closeDisabled={Boolean(issued)}
      onClose={onClose}
    >
      {issued ? (
        <DialogContent>
          <IssuedSetupLink issued={issued} onDone={onClose} />
        </DialogContent>
      ) : (
        <form
          onSubmit={(event) => {
            void submit(event);
          }}
        >
          <DialogContent>
            <fieldset className="storage-form" disabled={pending || unknown}>
              <TextField
                name="username"
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
                type="password"
                autoComplete="current-password"
                required
                label={"Current password"}
              />
            </fieldset>
            {error && (
              <p role="alert" className="form-error">
                {error}
              </p>
            )}
          </DialogContent>
          <DialogActions>
            <Button type="button" disabled={pending} onClick={onClose}>
              {unknown ? "Close and review" : "Cancel"}
            </Button>
            <Button
              type="submit"
              variant="contained"
              className="primary"
              disabled={pending || unknown}
            >
              {pending ? "Issuing..." : "Issue link"}
            </Button>
          </DialogActions>
        </form>
      )}
    </Dialog>
  );
}
