import { ClientUsage } from "../clients/model";

export function foldConnections<T>(
  rows: T[],
  id: (row: T) => string,
  pinned?: string,
) {
  const ordered = [...rows].sort((a, b) =>
    id(a).localeCompare(id(b), "en", { numeric: true }),
  );
  const visible = ordered.slice(0, rows.length > 6 ? 5 : 6);
  const selected = ordered.find((row) => id(row) === pinned);
  if (selected && !visible.includes(selected)) {
    visible[visible.length - 1] = selected;
    visible.sort((a, b) => id(a).localeCompare(id(b), "en", { numeric: true }));
  }
  const shown = new Set(visible.map(id));
  return { visible, hidden: ordered.filter((row) => !shown.has(id(row))) };
}

export type ClientTotals = { files: number; bytes: number };
export function clientTotals(rows: ClientUsage[]) {
  const result = new Map<string, ClientTotals>();
  for (const row of rows) {
    const value = result.get(row.client_id) ?? { files: 0, bytes: 0 };
    result.set(row.client_id, {
      files: value.files + row.active_files,
      bytes: value.bytes + row.active_bytes,
    });
  }
  return result;
}
export function sumClients(ids: string[], totals?: Map<string, ClientTotals>) {
  if (!totals) return undefined;
  return ids.reduce(
    (sum, id) => ({
      files: sum.files + (totals.get(id)?.files ?? 0),
      bytes: sum.bytes + (totals.get(id)?.bytes ?? 0),
    }),
    { files: 0, bytes: 0 },
  );
}
