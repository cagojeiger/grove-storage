import { ApiError, send } from "./http";
import { validMetadata } from "../features/metadata/model";

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
    case "storage.metadata.show":
    case "client.metadata.show":
    case "storage.metadata.replace":
    case "client.metadata.replace": {
      if (!object(value) || !("id" in input) || value.id !== input.id || !validMetadata(value.metadata)) return false;
      const metadata = value.metadata;
      return !("metadata" in input) || (validMetadata(input.metadata)
        && Object.keys(input.metadata).length === Object.keys(metadata).length
        && Object.entries(input.metadata).every(([key, item]) => Object.hasOwn(metadata, key) && metadata[key] === item));
    }
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
    case "storage.test":
      return object(value) && "id" in input && value.id === input.id && value.state === "ok";
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
    case "credential.list":
      return (
        Array.isArray(value) &&
        value.every((v) => typeof v === "string" && /^[a-z0-9]{8,64}$/.test(v))
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
    case "usage.history":
      return Array.isArray(value) && value.every((row) =>
        object(row) && typeof row.day === "string" && /^\d{4}-\d{2}-\d{2}$/.test(row.day)
        && Number.isFinite(Date.parse(row.day)) && new Date(row.day).toISOString().slice(0, 10) === row.day
        && typeof row.storage_id === "string" && typeof row.client_id === "string"
        && (row.observed_at == null || (typeof row.observed_at === "string" && Number.isFinite(Date.parse(row.observed_at))))
        && ["active_files", "active_bytes"].every((key) => typeof row[key] === "number" && Number.isSafeInteger(row[key]) && row[key] >= 0));
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
