// Disposable sample state. Production validates and persists in PostgreSQL.
export function previewMetadata(command, input, row, success, failure) {
  if (!command.endsWith(".metadata.show") && !command.endsWith(".metadata.replace")) return false;
  if (!row) { failure(404, "not_found"); return true; }
  if (command.endsWith(".replace")) {
    const value = input.metadata;
    if (!value || typeof value !== "object" || Array.isArray(value)
      || !Object.entries(value).every(([key, item]) => !key.includes("\0") && typeof item === "string" && !item.includes("\0"))) {
      failure(400, "invalid_input"); return true;
    }
    const json = `{${Object.entries(value).map(([key, item]) => `${JSON.stringify(key)}: ${JSON.stringify(item)}`).join(", ")}}`;
    if (Buffer.byteLength(json) > 8192) { failure(400, "invalid_input"); return true; }
    row.metadata = value;
  }
  success({ id: input.id, metadata: row.metadata ?? {} });
  return true;
}
