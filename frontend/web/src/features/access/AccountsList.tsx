import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Plus, Search } from "lucide-react";
import { identityRequest } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { isAccountPage, useAccountList } from "./accountList";

export function AccountsList({ onCreate }: { onCreate: () => void }) {
  const listing = useAccountList();
  const cache = useQueryClient();
  const query = useQuery({
    queryKey: ["access", "accounts", listing.query],
    queryFn: async ({ signal }) => {
      try {
        return await identityRequest(`/accounts?${listing.query}`, isAccountPage, { signal });
      } catch (error) {
        if (error instanceof ApiError && error.status === 403)
          void cache.invalidateQueries({ queryKey: ["session"] });
        throw error;
      }
    },
  });
  const data = query.data;
  return <>
    <div className="list-toolbar resource-toolbar account-toolbar">
      <form onSubmit={event => {
        event.preventDefault();
        const value = new FormData(event.currentTarget).get("search");
        listing.update({ q: typeof value === "string" ? value.trim() : "" });
      }}>
        <label><span className="sr-only">Search accounts</span>
          <input key={listing.search} name="search" type="search" placeholder="Search accounts" aria-label="Search accounts" defaultValue={listing.search} maxLength={80} />
        </label>
        <button className="icon-button" title="Search" aria-label="Search"><Search size={18} /></button>
      </form>
      <label>Role<select aria-label="Account role" value={listing.role} onChange={e => listing.update({ role: e.target.value === "all" ? null : e.target.value })}>
        <option value="all">All roles</option><option value="admin">Admin</option><option value="writer">Writer</option><option value="reader">Reader</option>
      </select></label>
      <label>Status<select aria-label="Account status" value={listing.status} onChange={e => listing.update({ status: e.target.value })}>
        <option value="current">Current</option><option value="active">Active</option><option value="disabled">Disabled</option><option value="deleted">Deleted</option><option value="all">All statuses</option>
      </select></label>
      <button className="primary" disabled={!data || query.isError} onClick={onCreate}><Plus size={16} />Create user</button>
    </div>
    <div className="account-list">
      {query.isPending ? <p role="status">Loading accounts...</p> : query.isError ?
        <p role="alert">{message(query.error)} <button onClick={() => void query.refetch()}>Retry</button></p> : <>
          {data?.items.map(row => <button className="account-row" key={row.id} onClick={() => { window.location.hash = listing.href(`#accounts/${encodeURIComponent(row.id)}`); }}>
            <span><strong>{row.display_name}</strong><span className="muted">{row.id}</span></span>
            <span>{row.role}</span><span>{row.deleted_at ? "Deleted" : !row.is_active ? "Disabled" : row.password_ready ? "Active" : "Pending setup"}</span><ChevronRight size={16} />
          </button>)}
          {!data?.items.length && <p className="empty">No matching accounts.</p>}
        </>}
    </div>
    <div className="pagination">
      <label>Rows<select aria-label="Rows per page" value={listing.limit} onChange={e => listing.update({ limit: e.target.value })}>
        {[20, 50, 100].map(size => <option key={size}>{size}</option>)}
      </select></label>
      {data && !query.isError && <span className="muted">{data.items.length} accounts on this page</span>}
      <nav aria-label="Account pagination">
        <button className="icon-button" aria-label="Previous page" title="Previous page" disabled={query.isFetching || query.isError || !data?.previous_after} onClick={() => listing.update({ after: data?.previous_after ?? null })}><ChevronLeft size={16} /></button>
        <button className="icon-button" aria-label="Next page" title="Next page" disabled={query.isFetching || query.isError || !data?.next_before} onClick={() => listing.update({ before: data?.next_before ?? null })}><ChevronRight size={16} /></button>
      </nav>
    </div>
  </>;
}
