import { ApiError, send } from "./http";

export const commandPath = "/api/admin/console-commands/v1";
const statuses: Record<string, number> = {
  unauthorized: 401,
  forbidden: 403,
  not_found: 404,
  conflict: 409,
  unavailable: 503,
  internal: 500,
  invalid_response: 500,
  invalid_input: 400,
  unknown_command: 400,
  protocol_incompatible: 400,
  request_rejected: 400,
};

function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function storage(value: unknown): boolean {
  return (
    object(value) &&
    typeof value.id === "string" &&
    ["s3", "fs"].includes(String(value.kind)) &&
    typeof value.capacity_bytes === "number" &&
    typeof value.force_relay === "boolean" &&
    typeof value.force_path_style === "boolean" &&
    [
      "root_path",
      "endpoint",
      "public_endpoint",
      "region",
      "bucket",
      "access_key",
    ].every((key) => value[key] === null || typeof value[key] === "string")
  );
}
function client(value: unknown): boolean {
  return (
    object(value) &&
    typeof value.id === "string" &&
    typeof value.storage_id === "string"
  );
}

// Validate the outputs this UI consumes, including the target of each mutation.
function output(name: string, value: unknown, input: object): boolean {
  switch (name) {
    case "storage.list":
      return Array.isArray(value) && value.every(storage);
    case "storage.show":
    case "storage.create":
    case "storage.replace":
      return (
        storage(value) &&
        object(value) &&
        "id" in input &&
        value.id === input.id
      );
    case "storage.delete":
      return (
        object(value) &&
        value.resource === "storage" &&
        "id" in input &&
        value.id === input.id
      );
    case "client.list":
      return (
        Array.isArray(value) && value.every((id) => typeof id === "string")
      );
    case "client.show":
    case "client.create":
      return (
        client(value) &&
        object(value) &&
        "id" in input &&
        value.id === input.id &&
        (!("storage_id" in input) || value.storage_id === input.storage_id)
      );
    case "client.delete":
      return (
        object(value) &&
        value.resource === "client" &&
        "id" in input &&
        value.id === input.id
      );
    case "client-key.list":
      return (
        Array.isArray(value) &&
        value.every(
          (v) => typeof v === "string" && /^sha256:[a-f0-9]{64}$/.test(v),
        )
      );
    case "credential.list":
      return (
        Array.isArray(value) &&
        value.every((v) => typeof v === "string" && /^[a-z0-9]{8,64}$/.test(v))
      );
    case "client-key.register":
      return (
        object(value) &&
        "client_id" in input &&
        "key_hash" in input &&
        value.client_id === input.client_id &&
        value.key_hash === input.key_hash
      );
    case "client-key.delete":
      return (
        object(value) &&
        value.resource === "client-key" &&
        "client_id" in input &&
        "key_hash" in input &&
        value.client_id === input.client_id &&
        value.id === input.key_hash
      );
    case "credential.create":
      return (
        object(value) &&
        typeof value.access_key_id === "string" &&
        /^[a-z0-9]{8,64}$/.test(value.access_key_id) &&
        typeof value.secret_key === "string" &&
        value.secret_key.length > 0
      );
    case "credential.delete":
      return (
        object(value) &&
        value.resource === "credential" &&
        "client_id" in input &&
        "access_key_id" in input &&
        value.client_id === input.client_id &&
        value.id === input.access_key_id
      );
    case "usage.clients":
      return (
        Array.isArray(value) &&
        value.every(
          (v) =>
            object(v) &&
            typeof v.client_id === "string" &&
            typeof v.storage_id === "string" &&
            Number.isFinite(v.active_files) &&
            Number.isFinite(v.active_bytes),
        )
      );
    case "usage.storages":
      return (
        Array.isArray(value) &&
        value.every(
          (row: unknown) =>
            object(row) &&
            typeof row.storage_id === "string" &&
            typeof row.kind === "string" &&
            [
              "capacity_bytes",
              "active_bytes",
              "reserved_bytes",
              "purge_pending_bytes",
              "remaining_bytes",
              "active_files",
              "reserved_files",
              "purge_pending_files",
            ].every((key) => typeof row[key] === "number"),
        )
      );
    default:
      return false;
  }
}

export async function command<T>(
  name: string,
  input = {},
  signal?: AbortSignal,
): Promise<T> {
  const response = await send(commandPath, {
    method: "POST",
    body: JSON.stringify({ protocol: 1, command: name, input }),
    signal,
  });
  // Browser guards can reject before the command envelope is decoded.
  if (response.status === 401 || response.status === 403)
    throw new ApiError(response.status, null, "not_applied");
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    throw new ApiError(502, null, "unknown");
  }
  if (
    !object(body) ||
    body.protocol !== 1 ||
    typeof body.request_id !== "string" ||
    !/^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(body.request_id)
  )
    throw new ApiError(502, null, "unknown");
  if (!response.ok) {
    const error = body.error;
    if (
      object(error) &&
      typeof error.code === "string" &&
      statuses[error.code] === response.status &&
      (error.outcome === "not_applied" ||
        error.outcome === "applied" ||
        error.outcome === "unknown")
    )
      throw new ApiError(response.status, null, error.outcome);
    throw new ApiError(502, null, "unknown");
  }
  if (
    response.status !== 200 ||
    body.command !== name ||
    !("result" in body) ||
    "error" in body ||
    !output(name, body.result, input)
  )
    throw new ApiError(502, null, "unknown");
  return body.result as T;
}
