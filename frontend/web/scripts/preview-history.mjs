import { randomUUID } from "node:crypto";

// Ephemeral preview events, not production audit persistence.
export function previewHistory() {
  const streams = { audit: [], invocations: [], security: [] };
  let sequence = 0;
  return {
    record(stream, session, fields, requestId = randomUUID()) {
      const rows = streams[stream];
      rows.unshift({
        ...fields,
        context: {
          id: String(++sequence), created_at: new Date().toISOString(),
          actor_kind: session ? "user" : "anonymous",
          actor_id: session?.user_id ?? null, owner_user_id: null,
          credential_id: session?.credential_id ?? null,
          session_id: session?.session_id ?? null,
          request_id: requestId, surface: "console",
        },
      });
      if (rows.length > 500) rows.pop();
    },
    page(stream, session, params) {
      if (!Object.hasOwn(streams, stream)) return null;
      const rows = streams[stream].filter(({ context: c }) =>
        (session.role === "admin" || c.actor_id === session.user_id) &&
        (!params.get("account_id") || c.actor_id === params.get("account_id")) &&
        (!params.get("credential_id") || c.credential_id === params.get("credential_id")) &&
        (!params.get("before") || Number(c.id) < Number(params.get("before"))));
      const limit = Math.max(1, Math.min(100, Number(params.get("limit")) || 50));
      const items = rows.slice(0, limit);
      return { items, next_before: rows.length > limit ? items.at(-1).context.id : null };
    },
  };
}
