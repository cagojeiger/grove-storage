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
  account,
  action,
  onClose,
  onSaved,
}: {
  account?: Account;
  action: AccountAction;
  onClose: () => void;
  onSaved: (createdId?: string) => Promise<void>;
}) {
  const state = useAction();
  const [confirmation, setConfirmation] = useState("");
  const title =
    action === "create"
      ? "Create user"
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
                    action === "role"
                      ? { operation: "role", role: data.get("role") }
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
                defaultValue={account?.role ?? "reader"}
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
            </p>
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
