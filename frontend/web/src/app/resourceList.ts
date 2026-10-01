import { useCallback } from "react";
import { useRoute } from "./navigation";

function readState(route: string) {
  const query = route.split("?")[1] ?? "";
  const params = new URLSearchParams(query);
  const requested = Number(params.get("page") ?? 1);
  return {
    search: params.get("q") ?? "",
    sort: params.get("sort") === "desc" ? "desc" : "asc",
    size: [20, 50, 100].includes(Number(params.get("size")))
      ? Number(params.get("size"))
      : 20,
    page: Number.isSafeInteger(requested) && requested > 0 ? requested : 1,
  };
}

export function useResourceList() {
  const route = useRoute();
  const query = route.split("?")[1] ?? "";
  const state = readState(route);
  const href = useCallback(
    (target: string) => {
      return target + (query ? `?${query}` : "");
    },
    [query],
  );
  function update(values: Partial<typeof state>) {
    const current = window.location.hash.slice(1);
    const next = { ...readState(current), ...values };
    if ("search" in values || "sort" in values || "size" in values)
      next.page = 1;
    const p = new URLSearchParams();
    if (next.search) p.set("q", next.search);
    if (next.sort !== "asc") p.set("sort", next.sort);
    if (next.size !== 20) p.set("size", String(next.size));
    if (next.page !== 1) p.set("page", String(next.page));
    window.history.replaceState(
      window.history.state,
      "",
      `#${current.split("?")[0]}${p.size ? `?${p}` : ""}`,
    );
    window.dispatchEvent(new HashChangeEvent("hashchange"));
  }
  return { ...state, href, update };
}
export type ResourceListState = ReturnType<typeof useResourceList>;

export function paginate<T>(
  rows: T[],
  state: Pick<ResourceListState, "search" | "sort" | "size" | "page">,
  id: (row: T) => string,
  searchable = id,
) {
  const filtered = rows
    .filter((row) =>
      searchable(row).toLowerCase().includes(state.search.toLowerCase()),
    )
    .sort(
      (a, b) =>
        id(a).localeCompare(id(b), "en", { numeric: true }) *
        (state.sort === "desc" ? -1 : 1),
    );
  const pages = Math.max(1, Math.ceil(filtered.length / state.size));
  const page = Math.min(state.page, pages);
  return {
    rows: filtered.slice((page - 1) * state.size, page * state.size),
    total: filtered.length,
    pages,
    page,
    size: state.size,
  };
}
