import { QueryClient } from "@tanstack/react-query";
import { ApiError } from "../../api/http";

export type Client = { id: string; storage_id: string };
export type ClientUsage = {
  client_id: string;
  storage_id: string;
  active_files: number;
  active_bytes: number;
};
export function totals(rows: ClientUsage[], id: string) {
  return rows
    .filter((row) => row.client_id === id)
    .reduce(
      (sum, row) => ({
        files: sum.files + row.active_files,
        bytes: sum.bytes + row.active_bytes,
      }),
      { files: 0, bytes: 0 },
    );
}
export function clientMessage(error: unknown): string {
  if (!(error instanceof ApiError) || error.outcome !== "not_applied")
    return "The outcome is unknown. Close and refresh before making another change. A key may have been created; review the key list before issuing again.";
  if (error.status === 409)
    return "Change blocked. The ID or key may already exist, or the client still has files, uploads or pending cleanup. Refresh and review the current state.";
  if (error.status === 404)
    return "The client, storage or key is unavailable. Refresh the list.";
  if (error.status === 400) return "Check the client ID, storage and key.";
  return error.message;
}
export async function refreshClients(cache: QueryClient) {
  await cache.invalidateQueries({
    predicate: (query) =>
      ["clients", "overview", "storage-usage"].includes(
        String(query.queryKey[0]),
      ),
  });
}
export const clientLink = (id: string) => `#clients/${encodeURIComponent(id)}`;
