import { useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  ChevronRight,
  Plus,
  RefreshCw,
  Shield,
  Trash2,
} from "lucide-react";
import { Account, identityPage, identityRequest, isAccount } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { AccountAction, AccountDialog } from "./AccountDialog";
import { Tokens } from "./Tokens";
import { RootAccount } from "./RootAccount";

export function Access({ route }: { route: string }) {
  const cache = useQueryClient();
  const selected = route.startsWith("accounts/") ? route.slice("accounts/".length) : null;
  const [action, setAction] = useState<AccountAction | null>(null);
  const [search, setSearch] = useState("");
  const query = useInfiniteQuery({
    queryKey: ["access", "accounts"],
    enabled: !selected,
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam, signal }) => {
      try {
        return await identityPage("/accounts", isAccount, pageParam, signal);
      } catch (e) {
        if (e instanceof ApiError && e.status === 403)
          void cache.invalidateQueries({ queryKey: ["session"] });
        throw e;
      }
    },
    getNextPageParam: (page) => page.next_before ?? undefined,
  });
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
  const accounts = query.data?.pages.flatMap((page) => page.items) ?? [];
  const account = detail.data;
  const current = selected ? detail : query;
  const rows = accounts.filter(
    (row) =>
      !row.deleted_at &&
      `${row.display_name} ${row.id}`
        .toLowerCase()
        .includes(search.toLowerCase()),
  );
  async function refresh(createdId?: string) {
    if (createdId) window.location.hash = `accounts/${encodeURIComponent(createdId)}`;
    await cache.invalidateQueries({ queryKey: ["access"] });
    await cache.invalidateQueries({ queryKey: ["session"] });
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
      {selected === "root" ? (
        <><button className="back-link" onClick={() => { window.location.hash = "accounts"; }}><ArrowLeft size={16} />Accounts</button><RootAccount selected onSelect={() => {}} /></>
      ) : current.isPending ? (
        <p role="status">Loading accounts...</p>
      ) : current.isError ? (
        <p role="alert">
          {current.error instanceof ApiError && current.error.status === 404 ? "Account unavailable." : message(current.error)}{" "}
          <button onClick={() => void current.refetch()}>Retry</button>
          {selected && <a href="#accounts">Back to accounts</a>}
        </p>
      ) : selected ? (
        account ? (
          <>
            <button className="back-link" onClick={() => { window.location.hash = "accounts"; }}>
              <ArrowLeft size={16} />
              Accounts
            </button>
            <div className="section-heading">
              <h2>{account.display_name}</h2>
              <div className="page-actions">
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
            <Tokens key={account.id} account={account} />
          </>
        ) : (
          <p>
            Account unavailable.{" "}
            <button onClick={() => { window.location.hash = "accounts"; }}>Back to accounts</button>
          </p>
        )
      ) : (
        <>
          <div className="list-toolbar">
            <label>
              <span className="sr-only">Search accounts</span>
              <input
                type="search"
                placeholder="Search accounts"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </label>
            {!accounts.length && !query.hasNextPage ? (
              <a className="back-link" href="#setup">Set up first Admin</a>
            ) : (
              <button className="primary" onClick={() => setAction("create")}>
                <Plus size={16} />
                Create user
              </button>
            )}
          </div>
          <div className="account-list">
            {(!search || "root config protected".includes(search.toLowerCase())) && <RootAccount selected={false} onSelect={() => { window.location.hash = "accounts/root"; }} />}
            {rows.map((row) => (
              <button
                className="account-row"
                key={row.id}
                onClick={() => { window.location.hash = `accounts/${encodeURIComponent(row.id)}`; }}
              >
                <span>
                  <strong>{row.display_name}</strong>
                  <span className="muted">{row.id}</span>
                </span>
                <span>{row.role}</span>
                <span>{row.is_active ? "Active" : "Disabled"}</span>
                <ChevronRight size={16} />
              </button>
            ))}
          </div>
          {!rows.length && <p className="empty">No users on loaded pages.</p>}
        </>
      )}
      {!selected && !query.isError && query.hasNextPage && (
        <button
          className="load-more"
          disabled={query.isFetchingNextPage}
          onClick={() => void query.fetchNextPage()}
        >
          Load more accounts
        </button>
      )}
      {action && !current.isError && (
        <AccountDialog
          account={account}
          action={action}
          onClose={() => setAction(null)}
          onSaved={refresh}
        />
      )}
    </main>
  );
}
