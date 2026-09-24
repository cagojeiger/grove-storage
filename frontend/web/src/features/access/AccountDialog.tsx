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

export type AccountAction = "create" | "role" | "active" | "delete";
export function AccountDialog({
  kind,
  account,
  action,
  owners,
  onClose,
  onSaved,
}: {
  kind: "user" | "agent";
  account?: Account;
  action: AccountAction;
  owners: Account[];
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const state = useAction();
  const [confirmation, setConfirmation] = useState("");
  const title =
    action === "create"
      ? `Create ${kind}`
      : action === "role"
        ? "Change role"
        : action === "delete"
          ? "Delete account"
          : account?.is_active
            ? "Disable account"
            : "Enable account";
  const dangerous =
    action === "delete" || (action === "active" && account?.is_active);
  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    if (dangerous && confirmation !== account?.display_name) return;
    await state.run(async () => {
      if (action === "create") {
        await identityRequest("/accounts", isCreated, {
          method: "POST",
          body: JSON.stringify({
            kind,
            display_name: field(data, "display_name").trim(),
            role: data.get("role"),
            ...(kind === "agent"
              ? { owner_user_id: data.get("owner_user_id") }
              : {}),
          }),
        });
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
                    action === "role"
                      ? { operation: "role", role: data.get("role") }
                      : { operation: "active", is_active: !account.is_active },
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
    <Dialog title={title} busy={state.busy} onClose={onClose}>
      <form onSubmit={(e) => void submit(e)}>
        <fieldset
          className="storage-form"
          disabled={state.busy || state.unknown}
        >
          {action === "create" ? (
            <label className="full-field">
              Name
              <input name="display_name" required maxLength={80} />
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
                defaultValue={account?.role ?? "viewer"}
              >
                <option value="viewer">Viewer</option>
                <option value="operator">Operator</option>
                {kind === "user" && <option value="admin">Admin</option>}
              </select>
            </label>
          )}
          {action === "create" && kind === "agent" && (
            <label>
              Owner
              <select
                name="owner_user_id"
                aria-label="Owner"
                required
                defaultValue=""
              >
                <option value="" disabled>
                  Select user
                </option>
                {owners.map((owner) => (
                  <option key={owner.id} value={owner.id}>
                    {owner.display_name} ({owner.id})
                  </option>
                ))}
              </select>
            </label>
          )}
          {action === "delete" && (
            <p className="full-field danger">
              Account tokens will be revoked. Deleting a User also revokes its
              Agents' tokens.
            </p>
          )}
          {action === "active" && account?.is_active && (
            <p className="full-field danger">
              Access is suspended. User sessions are revoked and owned Agents
              lose access.
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
