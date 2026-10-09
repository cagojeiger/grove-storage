export class ApiError extends Error {
  constructor(
    public status: number,
    public retryAfter: number | null = null,
    public outcome?: "not_applied" | "applied" | "unknown",
  ) {
    super(
      status === 401
        ? "Sign in to continue."
        : status === 403
          ? "You do not have permission to perform this action."
          : status === 429
            ? "Too many requests. Please try again later."
            : "The request could not be completed. Please try again.",
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
  credential_id: string | null;
  session_id: string;
  role: "reader" | "writer" | "admin";
};

export async function currentSession(signal?: AbortSignal): Promise<Session> {
  const value = await request<Session>(`${identity}/session`, { signal });
  if (!value || value.principal !== "user" || !["reader", "writer", "admin"].includes(value.role) || typeof value.session_id !== "string")
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
    : "Unable to connect to the server. Check your connection.";
}
