export type Snapshot = {
  day: string;
  storage_id: string;
  client_id: string;
  active_files: number;
  active_bytes: number;
};

export function dailyUsage(rows: Snapshot[]) {
  const totals = new Map<string, { files: number; bytes: number }>();
  for (const row of rows) {
    const current = totals.get(row.day) ?? { files: 0, bytes: 0 };
    totals.set(row.day, {
      files: current.files + row.active_files,
      bytes: current.bytes + row.active_bytes,
    });
  }
  const days = [...totals.keys()].sort();
  if (!days.length) return [];
  const first = Date.parse(`${days[0]}T00:00:00Z`);
  const last = Date.parse(`${days[days.length - 1]}T00:00:00Z`);
  const result: { day: string; files: number | null; bytes: number | null }[] =
    [];
  for (let stamp = first; stamp <= last; stamp += 86_400_000) {
    const day = new Date(stamp).toISOString().slice(0, 10);
    const recorded = totals.get(day);
    result.push({
      day,
      files: recorded?.files ?? null,
      bytes: recorded?.bytes ?? null,
    });
  }
  return result;
}
