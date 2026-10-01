export type Metadata = Record<string, string>;
export type ResourceMetadata = { id: string; metadata: Metadata };

export function validMetadata(value: unknown): value is Metadata {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const entries = Object.entries(value);
  if (!entries.every(([key, item]) => !key.includes("\0") && typeof item === "string" && !item.includes("\0"))) return false;
  const normalized = `{${entries.map(([key, item]) => `${JSON.stringify(key)}: ${JSON.stringify(item)}`).join(", ")}}`;
  return new TextEncoder().encode(normalized).length <= 8192;
}

export function parseMetadata(text: string): Metadata {
  let value: unknown;
  try { value = JSON.parse(text); }
  catch { throw new Error("Enter a valid JSON object."); }
  if (!validMetadata(value)) throw new Error("Use string values only, without null characters, within 8 KiB of normalized JSON.");
  return value;
}
