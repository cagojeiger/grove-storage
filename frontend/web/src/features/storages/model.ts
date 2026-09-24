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
      "용량은 0 이상의 정수 bytes여야 하며 9,007,199,254,740,991 bytes까지 입력할 수 있습니다.",
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
      throw new Error("Endpoint는 HTTP 또는 HTTPS 주소여야 합니다.");
    }
    if (
      !["http:", "https:"].includes(url.protocol) ||
      url.username ||
      url.password
    )
      throw new Error(
        "Endpoint는 사용자 인증 정보가 없는 HTTP 또는 HTTPS 주소여야 합니다.",
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
    return "변경 결과를 확인하지 못했습니다. 최신 목록과 상세를 확인한 후 다시 진행해 주세요.";
  if (error instanceof ApiError) {
    if (error.status === 409)
      return action === "delete"
        ? "연결된 클라이언트 또는 파일 위치가 남아 있어 삭제할 수 없습니다. 최신 상태를 확인해 주세요."
        : action === "replace"
          ? "파일 위치가 남아 있어 저장소 주소를 변경할 수 없습니다. 최신 상태를 확인해 주세요."
          : "이미 등록된 ID입니다. 최신 목록을 확인해 주세요.";
    if (error.status === 404)
      return "저장소가 더 이상 존재하지 않습니다. 최신 목록을 확인해 주세요.";
    if (error.status === 400 || error.status === 422)
      return "입력값과 저장소 접근 권한을 확인해 주세요. 등록 경로·버킷·인증 정보가 유효해야 합니다.";
    return error.message;
  }
  return "요청을 완료하지 못했습니다.";
}

export async function refreshStorages(cache: QueryClient) {
  await cache.invalidateQueries({
    predicate: (query) =>
      ["storages", "storage-usage", "overview"].includes(
        String(query.queryKey[0]),
      ),
  });
}
