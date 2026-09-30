import type { Usage } from "../../api/http";

export function accountedBytes(usage: Pick<Usage, "active_bytes" | "reserved_bytes" | "purge_pending_bytes">) {
  return usage.active_bytes + usage.reserved_bytes + usage.purge_pending_bytes;
}

export function capacityInput(value: number) {
  for (const [unit, scale] of [["TiB", 1024 ** 4], ["GiB", 1024 ** 3]] as const) {
    // Only choose a compact unit when its decimal representation round-trips exactly.
    const amount = value / scale;
    if (value > 0 && Number.isInteger(amount * 1000) && amount * scale === value)
      return { value: String(amount), unit };
  }
  return { value: String(value), unit: "B" };
}
