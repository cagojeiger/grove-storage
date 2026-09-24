import { useState } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  ChevronRight,
  Plus,
  RefreshCw,
  Shield,
  Trash2,
  UserRound,
  Bot,
} from "lucide-react";
import { identityPage, isAccount } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { AccountAction, AccountDialog } from "./AccountDialog";
import { Tokens } from "./Tokens";

export function Access({ kind }: { kind: "user" | "agent" }) {
  const cache = useQueryClient();
  const [selected, setSelected] = useState<string | null>(null);
  const [action, setAction] = useState<AccountAction | null>(null);
  const [search, setSearch] = useState("");
  const query = useInfiniteQuery({
    queryKey: ["access", "accounts"],
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
  const accounts = query.data?.pages.flatMap((page) => page.items) ?? [];
  const account = accounts.find((row) => row.id === selected);
  const rows = accounts.filter(
    (row) =>
      row.kind === kind &&
      !row.deleted_at &&
      `${row.display_name} ${row.id}`
        .toLowerCase()
        .includes(search.toLowerCase()),
  );
  const owners = accounts.filter(
    (row) => row.kind === "user" && row.is_active && !row.deleted_at,
  );
  async function refresh() {
    await cache.invalidateQueries({ queryKey: ["access"] });
    await cache.invalidateQueries({ queryKey: ["session"] });
  }
  return (
    <main className="overview storages access">
      <div className="page-heading">
        <div>
          <p className="eyebrow">ADMINISTRATION</p>
          <h1>Access</h1>
        </div>
        <button
          className="icon-button"
          title="Refresh access"
          aria-label="Refresh access"
          disabled={query.isFetching}
          onClick={() => void refresh()}
        >
          <RefreshCw size={18} />
        </button>
      </div>
      <nav className="access-tabs" aria-label="Access sections">
        <a
          href="#access/users"
          aria-current={kind === "user" ? "page" : undefined}
        >
          <UserRound size={16} />
          Users
        </a>
        <a
          href="#access/agents"
          aria-current={kind === "agent" ? "page" : undefined}
        >
          <Bot size={16} />
          Agents
        </a>
      </nav>
      {query.isPending ? (
        <p role="status">Loading accounts...</p>
      ) : query.isError ? (
        <p role="alert">
          {message(query.error)}{" "}
          <button onClick={() => void query.refetch()}>Retry</button>
        </p>
      ) : selected ? (
        account ? (
          <>
            <button className="back-link" onClick={() => setSelected(null)}>
              <ArrowLeft size={16} />
              {kind === "user" ? "Users" : "Agents"}
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
              {account.kind === "agent" && (
                <div>
                  <dt>Owner</dt>
                  <dd>
                    {accounts.find((row) => row.id === account.owner_user_id)
                      ?.display_name ?? account.owner_user_id}
                  </dd>
                </div>
              )}
            </dl>
            <Tokens key={account.id} account={account} />
          </>
        ) : (
          <p>
            Account unavailable.{" "}
            <button onClick={() => setSelected(null)}>Back to accounts</button>
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
            <button className="primary" onClick={() => setAction("create")}>
              <Plus size={16} />
              Create {kind}
            </button>
          </div>
          <div className="account-list">
            {rows.map((row) => (
              <button
                className="account-row"
                key={row.id}
                onClick={() => setSelected(row.id)}
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
          {!rows.length && (
            <p className="empty">
              No {kind === "user" ? "users" : "agents"} on loaded pages.
            </p>
          )}
        </>
      )}
      {!query.isError && query.hasNextPage && (
        <button
          className="load-more"
          disabled={query.isFetchingNextPage}
          onClick={() => void query.fetchNextPage()}
        >
          Load more accounts
        </button>
      )}
      {action && !query.isError && (
        <AccountDialog
          kind={kind}
          account={account}
          action={action}
          owners={owners}
          onClose={() => setAction(null)}
          onSaved={refresh}
        />
      )}
    </main>
  );
}
