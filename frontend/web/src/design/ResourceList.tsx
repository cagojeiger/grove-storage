import { ChevronLeft, ChevronRight } from "lucide-react";
import { ResourceListState } from "../app/resourceList";

export function ListToolbar({
  state,
  label,
}: {
  state: ResourceListState;
  label: string;
}) {
  return (
    <div className="list-toolbar resource-toolbar">
      <label>
        <span className="sr-only">{label}</span>
        <input
          type="search"
          placeholder={label}
          value={state.search}
          onChange={(e) => state.update({ search: e.target.value })}
        />
      </label>
      <label className="sort-control">
        Sort
        <select
          aria-label="Sort"
          value={state.sort}
          onChange={(e) => state.update({ sort: e.target.value })}
        >
          <option value="asc">Name A–Z</option>
          <option value="desc">Name Z–A</option>
        </select>
      </label>
    </div>
  );
}
export function Pagination({
  state,
  page,
  pages,
  total,
}: {
  state: ResourceListState;
  page: number;
  pages: number;
  total: number;
}) {
  const start = Math.max(1, Math.min(page - 2, pages - 4));
  const visible = [
    ...new Set([
      1,
      ...Array.from({ length: Math.min(5, pages) }, (_, i) => start + i),
      pages,
    ]),
  ];
  return (
    <div className="pagination">
      <label>
        Rows
        <select
          aria-label="Rows per page"
          value={state.size}
          onChange={(e) => state.update({ size: Number(e.target.value) })}
        >
          {[20, 50, 100].map((size) => (
            <option key={size}>{size}</option>
          ))}
        </select>
      </label>
      <span className="muted" role="status">
        {total ? (page - 1) * state.size + 1 : 0}–
        {Math.min(page * state.size, total)} of {total.toLocaleString("en-US")}
      </span>
      <nav aria-label="Pagination">
        <button
          className="icon-button"
          title="Previous page"
          aria-label="Previous page"
          disabled={page === 1}
          onClick={() => state.update({ page: page - 1 })}
        >
          <ChevronLeft size={16} />
        </button>
        {visible.map((n, i) => (
          <span className="page-number" key={n}>
            {i > 0 && n - visible[i - 1] > 1 && (
              <span aria-hidden="true">…</span>
            )}
            <button
              aria-label={`Page ${n}`}
              aria-current={page === n ? "page" : undefined}
              onClick={() => state.update({ page: n })}
            >
              {n}
            </button>
          </span>
        ))}
        <button
          className="icon-button"
          title="Next page"
          aria-label="Next page"
          disabled={page === pages}
          onClick={() => state.update({ page: page + 1 })}
        >
          <ChevronRight size={16} />
        </button>
      </nav>
    </div>
  );
}
