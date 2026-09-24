import { QueryClient } from "@tanstack/react-query";
import { ApiError } from "../../api/http";

export type Storage = {
  id: string;
  kind: "s3" | "fs";
  capacity_bytes: number;
  force_relay: boolean;
  root_path: string | null;
  endpoint: string | null;
  public_endpoint: string | null;
  region: string | null;
  bucket: string | null;
  force_path_style: boolean;
  access_key: string | null;
};

export type StorageSpec =
  | {
      kind: "fs";
      root_path: string;
      capacity_bytes: number;
    }
  | {
      kind: "s3";
      endpoint: string;
      public_endpoint?: string;
      region: string;
      bucket: string;
      access_key: string;
      secret_key: string;
      capacity_bytes: number;
      force_relay: boolean;
      force_path_style: boolean;
    };

export const idPattern = "[a-z0-9]([a-z0-9\\-]{0,62}[a-z0-9])?";

export function capacityBytes(value: string, unit: string): number {
  const multiplier: Record<string, bigint> = {
    B: 1n,
    GiB: 1024n ** 3n,
    TiB: 1024n ** 4n,
  };
  const invalid = () =>
    new Error(
      "Capacity must be a whole number of bytes between 0 and 9,007,199,254,740,991.",
    );
  const match = /^(\d+)(?:\.(\d+))?$/.exec(value);
  if (!match || value.length > 64 || !Object.hasOwn(multiplier, unit))
    throw invalid();
  // Convert decimal input exactly before crossing the JSON number boundary.
  const fraction = match[2] ?? "";
  const numerator = BigInt(match[1] + fraction) * multiplier[unit];
  const denominator = 10n ** BigInt(fraction.length);
  const result = numerator / denominator;
  if (
    numerator % denominator !== 0n ||
    result > BigInt(Number.MAX_SAFE_INTEGER)
  )
    throw invalid();
  return Number(result);
}

export function storageSpec(
  data: FormData,
  kind: Storage["kind"],
): StorageSpec {
  const field = (name: string) => {
    const value = data.get(name);
    return typeof value === "string" ? value : "";
  };
  const capacity_bytes = capacityBytes(field("capacity"), field("unit"));
  if (kind === "fs")
    return { kind, root_path: field("root_path").trim(), capacity_bytes };
  const endpoint = field("endpoint").trim();
  const public_endpoint = field("public_endpoint").trim();
  for (const value of [endpoint, public_endpoint].filter(Boolean)) {
    let url: URL;
    try {
      url = new URL(value);
    } catch {
      throw new Error("Endpoint must be an HTTP or HTTPS URL.");
    }
    if (
      !["http:", "https:"].includes(url.protocol) ||
      url.username ||
      url.password
    )
      throw new Error(
        "Endpoint must be an HTTP or HTTPS URL without embedded credentials.",
      );
  }
  return {
    kind,
    endpoint,
    ...(public_endpoint ? { public_endpoint } : {}),
    region: field("region").trim(),
    bucket: field("bucket").trim(),
    access_key: field("access_key").trim(),
    secret_key: field("secret_key"),
    capacity_bytes,
    force_relay: data.has("force_relay"),
    force_path_style: data.has("force_path_style"),
  };
}

export function uncertain(error: unknown) {
  if (error instanceof ApiError && error.outcome)
    return error.outcome !== "not_applied";
  return (
    !(error instanceof ApiError) || error.status >= 500 || error.status === 408
  );
}

export function mutationMessage(
  error: unknown,
  action: "create" | "replace" | "delete",
) {
  if (uncertain(error))
    return "The change outcome is unconfirmed. Check the latest list and details before trying again.";
  if (error instanceof ApiError) {
    if (error.status === 409)
      return action === "delete"
        ? "Storage is referenced by clients or file locations. Refresh and try again."
        : action === "replace"
          ? "The storage address cannot be changed while file locations exist. Refresh and try again."
          : "This ID is already registered. Refresh the list.";
    if (error.status === 404)
      return "This storage no longer exists. Refresh the list.";
    if (error.status === 400 || error.status === 422)
      return "Check the input and storage access. The path, bucket, and credentials must be valid.";
    return error.message;
  }
  return "The request could not be completed.";
}

export async function refreshStorages(cache: QueryClient) {
  await cache.invalidateQueries({
    predicate: (query) =>
      ["storages", "storage-usage", "overview"].includes(
        String(query.queryKey[0]),
      ),
  });
}
