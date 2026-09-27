import { FormEvent, useState } from "react";
import {
  Account,
  field,
  identityRequest,
  isChanged,
  isCreated,
} from "../../api/identity";
import { Dialog } from "../../design/Dialog";
import { useAction } from "./useAction";

export type AccountAction = "create" | "name" | "role" | "active" | "delete";
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
  onSaved: (createdId?: string) => Promise<void>;
}) {
  const state = useAction();
  const [confirmation, setConfirmation] = useState("");
  const [name, setName] = useState(
    action === "name" ? account?.display_name ?? "" : "",
  );
  const [role, setRole] = useState(account?.role ?? "reader");
  const [acknowledged, setAcknowledged] = useState(false);
  const title =
    action === "create"
      ? "Create user"
      : action === "name"
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
    isSelf && action === "role" && account?.role === "admin" && role !== "admin";
  const selfImpact = isSelf && (dangerous || selfDemotion);
  const invalidName =
    (action === "create" || action === "name") &&
    (!name.trim() || Array.from(name.trim()).length > 80 ||
      (action === "name" && name.trim() === account?.display_name));
  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    if (dangerous && confirmation !== account?.display_name) return;
    if (invalidName || (selfImpact && !acknowledged)) return;
    await state.run(async () => {
      let createdId: string | undefined;
      if (action === "create") {
        const created = await identityRequest("/accounts", isCreated, {
          method: "POST",
          body: JSON.stringify({
            kind: "user",
            display_name: field(data, "display_name").trim(),
            role: data.get("role"),
          }),
        });
        createdId = created.account_id;
      } else if (account) {
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
                        : { operation: "active", is_active: !account.is_active },
                  ),
                }),
          },
        );
      }
      onClose();
      await onSaved(createdId);
    });
  }
  return (
    <Dialog title={title} busy={state.busy} onClose={onClose}>
      <form onSubmit={(e) => void submit(e)}>
        <fieldset
          className="storage-form"
          disabled={state.busy || state.unknown}
        >
          {action === "create" || action === "name" ? (
            <label className="full-field">
              Name
              <input
                name="display_name"
                required
                maxLength={80}
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
          ) : (
            <p className="full-field">{account?.display_name}</p>
          )}
          {(action === "create" || action === "role") && (
            <label>
              Role
              <select
                name="role"
                aria-label="Role"
                value={role}
                onChange={(e) => {
                  setRole(e.target.value as Account["role"]);
                  setAcknowledged(false);
                }}
              >
                <option value="reader">Reader</option>
                <option value="writer">Writer</option>
                <option value="admin">Admin</option>
              </select>
            </label>
          )}
          {action === "delete" && (
            <p className="full-field danger">
              All tokens and sessions belonging to this User will be revoked.
              Storage, clients, and files are preserved.
            </p>
          )}
          {action === "role" && (
            <p className="full-field">
              The selected role applies to existing tokens and sessions.
            </p>
          )}
          {action === "active" && !account?.is_active && (
            <p className="full-field">
              Unexpired, unrevoked tokens become usable again. Previous sessions remain revoked.
            </p>
          )}
          {selfImpact && (
            <label className="full-field check-field account-confirmation">
              <input
                type="checkbox"
                checked={acknowledged}
                onChange={(e) => setAcknowledged(e.target.checked)}
              />
              {selfDemotion
                ? "I understand I will lose access to Accounts."
                : "I understand my current session will end."}
            </label>
          )}
          {action === "active" && account?.is_active && (
            <p className="full-field danger">
              Access is suspended for all of this User's tokens. Existing
              sessions are revoked.
            </p>
          )}
          {dangerous && (
            <label className="full-field">
              Confirm account name
              <input
                value={confirmation}
                onChange={(e) => setConfirmation(e.target.value)}
                autoComplete="off"
                required
              />
            </label>
          )}
        </fieldset>
        {state.error && (
          <p role="alert" className="form-error">
            {state.error}
          </p>
        )}
        <div className="dialog-actions">
          <button type="button" disabled={state.busy} onClick={onClose}>
            {state.unknown ? "Close and review" : "Cancel"}
          </button>
          <button
            className="primary"
            disabled={
              state.busy ||
              state.unknown ||
              invalidName ||
              (selfImpact && !acknowledged) ||
              Boolean(dangerous && confirmation !== account?.display_name)
            }
          >
            {state.busy ? "Saving..." : "Confirm"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
