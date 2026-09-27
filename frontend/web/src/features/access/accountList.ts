import { useRoute } from "../../app/navigation";
import { Account, isAccount, isObject } from "../../api/identity";

export type AccountPage = {
  items: Account[];
  next_before: string | null;
  previous_after: string | null;
  initialized: boolean;
};
export const isAccountPage = (value: unknown): value is AccountPage =>
  isObject(value) && Array.isArray(value.items) && value.items.every(isAccount) &&
  [value.next_before, value.previous_after].every(v => v === null || typeof v === "string") &&
  typeof value.initialized === "boolean";

export function useAccountList() {
  const route = useRoute();
  const supplied = new URLSearchParams(route.split("?")[1] ?? "");
  const params = new URLSearchParams();
  const search = supplied.get("q") ?? "";
  const role = supplied.get("role") ?? "all";
  const status = supplied.get("status") ?? "current";
  const limit = supplied.get("limit") ?? "50";
  if (search) params.set("q", search);
  if (role !== "all") params.set("role", role);
  params.set("status", status);
  params.set("limit", limit);
  for (const key of ["before", "after"]) {
    const value = supplied.get(key);
    if (value) params.set(key, value);
  }
  const query = params.toString();
  function href(target: string) { return `${target}?${query}`; }
  function update(values: Record<string, string | null>) {
    const next = new URLSearchParams(query);
    next.delete("before");
    next.delete("after");
    for (const [key, value] of Object.entries(values)) {
      if (value) next.set(key, value);
      else next.delete(key);
    }
    window.location.hash = `accounts?${next}`;
  }
  return { search, role, status, limit, query, href, update };
}
