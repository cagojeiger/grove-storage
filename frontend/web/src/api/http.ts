export class ApiError extends Error {
  constructor(
    public status: number,
    public retryAfter: number | null = null,
    public outcome?: "not_applied" | "applied" | "unknown",
  ) {
    super(
      status === 401
        ? "로그인이 필요합니다."
        : status === 403
          ? "이 작업을 수행할 권한이 없습니다."
          : status === 429
            ? "로그인 요청이 많습니다. 잠시 후 다시 시도하세요."
            : "요청을 완료하지 못했습니다. 다시 시도해 주세요.",
    );
  }
}

export async function send(
  path: string,
  options: RequestInit = {},
): Promise<Response> {
  const headers = new Headers(options.headers);
  if (options.body) headers.set("Content-Type", "application/json");
  if (options.method && options.method !== "GET")
    headers.set("X-Grove-CSRF", "1");
  // Bound reads and writes; mutations are never automatically retried.
  const timeout = AbortSignal.timeout(15000);
  const signal = options.signal
    ? AbortSignal.any([options.signal, timeout])
    : timeout;
  return fetch(path, {
    ...options,
    headers,
    signal,
    credentials: "same-origin",
    cache: "no-store",
    // A 307/308 must not replay passwords or provider secrets at another URL.
    redirect: "error",
  });
}

export async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await send(path, options);
  if (!response.ok) {
    const seconds = Number(response.headers.get("Retry-After"));
    throw new ApiError(response.status, seconds > 0 ? seconds : null);
  }
  // The API contract supplies T; this assertion is not runtime schema validation.
  const payload: unknown = response.status === 204 ? undefined : await response.json();
  return payload as T;
}

export const identity = "/api/admin/identity/v1";
export type Session = {
  principal: "user";
  user_id: string;
  credential_id: string;
  session_id: string;
  role: "viewer" | "operator" | "admin";
};

export async function currentSession(signal?: AbortSignal): Promise<Session> {
  const value = await request<Session>(`${identity}/session`, { signal });
  if (!value || value.principal !== "user" ||
      !["viewer", "operator", "admin"].includes(value.role))
    throw new ApiError(502);
  return value;
}
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
