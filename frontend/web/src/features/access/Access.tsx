import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  RefreshCw,
  Pencil,
  History,
  Shield,
  Trash2,
} from "lucide-react";
import { Account, identityRequest, isAccount } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { AccountAction, AccountDialog } from "./AccountDialog";
import { Tokens } from "./Tokens";
import { PasswordSetupIssue } from "./PasswordSetupIssue";
import { RootAccount } from "./RootAccount";
import { AccountsList } from "./AccountsList";
import { useAccountList } from "./accountList";
import { activityLink } from "../activity/filters";

export function Access({ route, currentUserId }: { route: string; currentUserId?: string }) {
  const cache = useQueryClient();
  const listing = useAccountList();
  const selected = route.startsWith("accounts/") ? route.slice("accounts/".length) : null;
  const [action, setAction] = useState<AccountAction | null>(null);
  const detail = useQuery({
    queryKey: ["access", "account", selected],
    enabled: Boolean(selected && selected !== "root"),
    queryFn: async ({ signal }) => {
      try {
        return await identityRequest(
          `/accounts/${encodeURIComponent(selected ?? "")}`,
          (v): v is Account => isAccount(v) && v.id === selected,
          { signal },
        );
      } catch (error) {
        if (error instanceof ApiError && error.status === 403)
          void cache.invalidateQueries({ queryKey: ["session"] });
        throw error;
      }
    },
  });
  const account = detail.data;
  const current = detail;
  async function refresh(createdId?: string) {
    if (createdId) window.location.hash = listing.href(`#accounts/${encodeURIComponent(createdId)}`);
    await cache.invalidateQueries({ queryKey: ["session"] });
    await cache.invalidateQueries({ queryKey: ["access"] });
  }
  return (
    <main className="overview storages access">
      <div className="page-heading">
        <div>
          <p className="eyebrow">ADMINISTRATION</p>
          <h1>Accounts</h1>
        </div>
        <button
          className="icon-button"
          title="Refresh accounts"
          aria-label="Refresh accounts"
          disabled={current.isFetching}
          onClick={() => void refresh()}
        >
          <RefreshCw size={18} />
        </button>
      </div>
      {!selected ? <AccountsList onCreate={() => setAction("create")} /> : selected === "root" ? (
        <><button className="back-link" onClick={() => { window.location.hash = listing.href("#accounts"); }}><ArrowLeft size={16} />Accounts</button><RootAccount selected onSelect={() => {}} /></>
      ) : current.isPending ? (
        <p role="status">Loading accounts...</p>
      ) : current.isError ? (
        <p role="alert">
          {current.error instanceof ApiError && current.error.status === 404 ? "Account unavailable." : message(current.error)}{" "}
          <button onClick={() => void current.refetch()}>Retry</button>
          <a href={listing.href("#accounts")}>Back to accounts</a>
        </p>
      ) : selected ? (
        account ? (
          <>
            <button className="back-link" onClick={() => { window.location.hash = listing.href("#accounts"); }}>
              <ArrowLeft size={16} />
              Accounts
            </button>
            <div className="section-heading">
              <h2>{account.display_name}</h2>
              <div className="page-actions">
                <a className="icon-button" href={activityLink(account.id)} title="View account actions" aria-label="View account actions">
                  <History size={16} />
                </a>
                <button
                  className="icon-button"
                  title="Edit name"
                  aria-label="Edit name"
                  disabled={Boolean(account.deleted_at)}
                  onClick={() => setAction("name")}
                >
                  <Pencil size={16} />
                </button>
                <button
                  className="action-button"
                  disabled={Boolean(account.deleted_at)}
                  onClick={() => setAction("role")}
                >
                  <Shield size={16} />
                  Change role
                </button>
                <button
                  disabled={Boolean(account.deleted_at)}
                  onClick={() => setAction("active")}
                >
                  {account.is_active ? "Disable" : "Enable"}
                </button>
                <button
                  className="icon-button"
                  title="Delete account"
                  aria-label="Delete account"
                  disabled={Boolean(account.deleted_at)}
                  onClick={() => setAction("delete")}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </div>
            <dl className="detail-fields">
              <div>
                <dt>Account ID</dt>
                <dd>{account.id}</dd>
              </div>
              <div>
                <dt>Role</dt>
                <dd>{account.role}</dd>
              </div>
              <div>
                <dt>Status</dt>
                <dd>
                  {account.deleted_at
                    ? "Deleted"
                    : account.is_active
                      ? "Active"
                      : "Disabled"}
                </dd>
              </div>
            </dl>
            <PasswordSetupIssue key={account.id} account={account} />
            <Tokens key={account.id} account={account} />
          </>
        ) : (
          <p>
            Account unavailable.{" "}
            <button onClick={() => { window.location.hash = listing.href("#accounts"); }}>Back to accounts</button>
          </p>
        )
      ) : null}
      {action && !current.isError && (
        <AccountDialog
          account={account}
          isSelf={Boolean(account && account.id === currentUserId)}
          action={action}
          onClose={() => setAction(null)}
          onSaved={refresh}
        />
      )}
    </main>
  );
}
