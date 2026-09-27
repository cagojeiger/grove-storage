import { FormEvent, useState } from "react";
import { Copy, Link2 } from "lucide-react";
import { identity, ApiError, message, request } from "../../api/http";
import { field, Account } from "../../api/identity";
import { Dialog } from "../../design/Dialog";

type IssuedSetup = { account_id: string; username: string; expires_at: string; token: string };
function isIssuedSetup(value: unknown, account: string): value is IssuedSetup {
  if (!value || typeof value !== "object") return false;
  const result = value as Record<string, unknown>;
  return result.account_id === account && typeof result.username === "string"
    && typeof result.expires_at === "string" && Number.isFinite(Date.parse(result.expires_at))
    && typeof result.token === "string" && /^gsps_[0-9a-f]{64}$/.test(result.token);
}

export function PasswordSetupIssue({ account }: { account: Account }) {
  const [open, setOpen] = useState(false);
  return (
    <section className="storage-section" aria-label="Password setup">
      <div className="section-heading">
        <h2>Password setup</h2>
        <button className="action-button" disabled={!account.is_active || Boolean(account.deleted_at)} onClick={() => setOpen(true)}>
          <Link2 size={16} aria-hidden="true" />Issue setup link
        </button>
      </div>
      {open && <SetupDialog account={account} onClose={() => setOpen(false)} />}
    </section>
  );
}

function SetupDialog({ account, onClose }: { account: Account; onClose: () => void }) {
  const [pending, setPending] = useState(false);
  const [unknown, setUnknown] = useState(false);
  const [error, setError] = useState("");
  const [issued, setIssued] = useState<IssuedSetup | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const [copyError, setCopyError] = useState("");
  const link = issued ? new URL(`${import.meta.env.BASE_URL}#set-password/${issued.token}`, window.location.origin).toString() : "";

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    form.reset();
    if (pending || unknown) return;
    setPending(true);
    setError("");
    try {
      const result = await request<unknown>(`${identity}/accounts/${encodeURIComponent(account.id)}/password-setup`, {
        method: "POST",
        body: JSON.stringify({ username: field(data, "username").trim(), current_password: field(data, "current_password") }),
      });
      if (!isIssuedSetup(result, account.id)) throw new Error("Invalid setup response");
      setIssued(result);
    } catch (failure) {
      if (failure instanceof ApiError && [400, 401, 403, 404, 409, 429].includes(failure.status)) {
        setError(failure.status === 401 ? "Current password is incorrect or the session expired."
          : failure.status === 409 ? "The username is in use or this account already has a password."
            : failure.status === 400 ? "Enter a valid username."
              : message(failure));
      } else {
        setUnknown(true);
        setError("The result is unknown. Close and refresh this account before issuing another link.");
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog title={issued ? "Save setup link" : "Issue setup link"} busy={pending} closeDisabled={Boolean(issued)} onClose={onClose}>
      {issued ? (
        <div className="storage-form">
          <p>Share this link privately with {issued.username}. It is shown once and expires in 24 hours.</p>
          <label className="full-field">Setup link
            <input aria-label="Setup link" value={link} readOnly onFocus={(event) => event.currentTarget.select()} />
          </label>
          <button type="button" onClick={() => {
            if (!navigator.clipboard) { setCopyError("Copy unavailable. Select the link manually."); return; }
            void navigator.clipboard.writeText(link).catch(() => setCopyError("Copy failed. Select the link manually."));
          }}><Copy size={16} aria-hidden="true" />Copy link</button>
          {copyError && <p role="alert">{copyError}</p>}
          <label className="check-field full-field"><input type="checkbox" checked={acknowledged} onChange={(event) => setAcknowledged(event.target.checked)} />I have saved this setup link.</label>
          <div className="dialog-actions"><button className="primary" disabled={!acknowledged} onClick={onClose}>Done</button></div>
        </div>
      ) : (
        <form onSubmit={(event) => { void submit(event); }}>
          <fieldset className="storage-form" disabled={pending || unknown}>
            <label>Username<input name="username" autoComplete="off" required minLength={3} maxLength={64} pattern="[A-Za-z0-9][A-Za-z0-9._-]{2,63}" /></label>
            <label>Current password<input name="current_password" type="password" autoComplete="current-password" required /></label>
          </fieldset>
          {error && <p role="alert" className="form-error">{error}</p>}
          <div className="dialog-actions">
            <button type="button" disabled={pending} onClick={onClose}>{unknown ? "Close and review" : "Cancel"}</button>
            <button className="primary" disabled={pending || unknown}>{pending ? "Issuing..." : "Issue link"}</button>
          </div>
        </form>
      )}
    </Dialog>
  );
}
