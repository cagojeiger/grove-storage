export class ApiError extends Error {
  constructor(
    public status: number,
    public retryAfter: number | null = null,
  ) {
    super(
      status === 401
        ? "로그인이 필요합니다."
        : status === 429
          ? "로그인 요청이 많습니다. 잠시 후 다시 시도하세요."
          : "요청을 완료하지 못했습니다. 다시 시도해 주세요.",
    );
  }
}

export async function request<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const headers = new Headers(options.headers);
  if (options.body) headers.set("Content-Type", "application/json");
  if (options.method && options.method !== "GET")
    headers.set("X-FileGate-CSRF", "1");
  // Bound reads and writes; mutations are never automatically retried.
  const timeout = AbortSignal.timeout(15000);
  const signal = options.signal
    ? AbortSignal.any([options.signal, timeout])
    : timeout;
  const response = await fetch(path, {
    ...options,
    headers,
    signal,
    credentials: "same-origin",
    cache: "no-store",
  });
  if (!response.ok) {
    const seconds = Number(response.headers.get("Retry-After"));
    throw new ApiError(response.status, seconds > 0 ? seconds : null);
  }
  // The API contract supplies T; this assertion is not runtime schema validation.
  const payload: unknown = response.status === 204 ? undefined : await response.json();
  return payload as T;
}

export const admin = "/api/admin/v1";
export type Session = { principal: string; credential_id: string };
export type Usage = {
  storage_id: string;
  kind: string;
  capacity_bytes: number;
  active_bytes: number;
  reserved_bytes: number;
  purge_pending_bytes: number;
  remaining_bytes: number;
  active_files: number;
  reserved_files: number;
  purge_pending_files: number;
};

export function message(error: unknown): string {
  return error instanceof ApiError
    ? error.message
    : "서버에 연결하지 못했습니다. 연결 상태를 확인해 주세요.";
}
