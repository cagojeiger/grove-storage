import { isObject } from "../../api/identity";

export type Stream = "audit" | "invocations" | "security";
export type EventContext = {
  id: string;
  created_at: string;
  actor_kind: string;
  actor_id: string | null;
  credential_id: string | null;
  session_id: string | null;
  request_id: string;
  surface: string;
};
export type Audit = {
  context: EventContext;
  action: string;
  resource_type: string;
  resource_id: string;
  metadata: Record<string, unknown>;
};
export type Invocation = {
  context: EventContext;
  operation: string;
  outcome: string;
  error_code: string | null;
  duration_ms: number;
};
export type Security = {
  context: EventContext;
  event_type: string;
  reason_code: string;
};
export type Event = Audit | Invocation | Security;
const nullable = (v: unknown) => v === null || typeof v === "string";
const date = (v: unknown) =>
  typeof v === "string" && Number.isFinite(Date.parse(v));
export function isContext(v: unknown): v is EventContext {
  return (
    isObject(v) &&
    ["id", "actor_kind", "request_id", "surface"].every(
      (k) => typeof v[k] === "string",
    ) &&
    date(v.created_at) &&
    ["actor_id", "credential_id", "session_id"].every((k) =>
      nullable(v[k]),
    )
  );
}
export function validator(stream: Stream): (v: unknown) => v is Event {
  return (v): v is Event => {
    if (!isObject(v) || !isContext(v.context)) return false;
    if (stream === "audit")
      return (
        ["action", "resource_type", "resource_id"].every(
          (k) => typeof v[k] === "string",
        ) && isObject(v.metadata)
      );
    if (stream === "invocations")
      return (
        typeof v.operation === "string" &&
        typeof v.outcome === "string" &&
        nullable(v.error_code) &&
        Number.isFinite(v.duration_ms)
      );
    return (
      typeof v.event_type === "string" && typeof v.reason_code === "string"
    );
  };
}
export const eventName = (event: Event) =>
  "action" in event
    ? event.action
    : "operation" in event
      ? event.operation
      : event.event_type;
export const eventResult = (event: Event) =>
  "action" in event
    ? `${event.resource_type}: ${event.resource_id}`
    : "outcome" in event
      ? event.outcome
      : event.reason_code;
export const actor = (context: EventContext) =>
  context.actor_kind === "system" ? "System" : (context.actor_id ?? "Unknown");
