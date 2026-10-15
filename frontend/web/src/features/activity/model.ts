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
  outcome: "succeeded" | "denied" | "failed" | "unknown";
  error_code: string | null;
  duration_ms: number;
  target_type?: string | null;
  target_id?: string | null;
  stage?: string | null;
  auth_method?: string | null;
};
export type Security = {
  context: EventContext;
  event_type: string;
  reason_code: string;
  operation?: string | null;
  stage?: string | null;
  auth_method?: string | null;
  is_sampled?: boolean | null;
};
export type Event = Audit | Invocation | Security;
const nullable = (v: unknown) => v === null || typeof v === "string";
const optionalText = (v: unknown) => v === undefined || nullable(v);
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
        Object.hasOwn(invocationOutcomes, v.outcome) &&
        nullable(v.error_code) &&
        typeof v.duration_ms === "number" &&
        Number.isSafeInteger(v.duration_ms) && v.duration_ms >= 0 &&
        ["target_type", "target_id", "stage", "auth_method"].every(k => optionalText(v[k]))
      );
    return (
      typeof v.event_type === "string" && typeof v.reason_code === "string" &&
      ["operation", "stage", "auth_method"].every(k => optionalText(v[k])) &&
      (v.is_sampled === undefined || v.is_sampled === null || typeof v.is_sampled === "boolean")
    );
  };
}
export const eventName = (event: Event) =>
  "action" in event
    ? event.action
    : "event_type" in event
      ? event.event_type
      : event.operation;
export const eventResult = (event: Event) =>
  "action" in event
    ? `${event.resource_type}: ${event.resource_id}`
    : "outcome" in event
      ? event.outcome
      : event.reason_code;
export const actor = (context: EventContext) =>
  context.actor_kind === "system" ? "System"
    : context.actor_kind === "anonymous" ? "Anonymous"
      : (context.actor_id ?? "Unknown");

const invocationOutcomes = {
  succeeded: { label: "Succeeded", color: "success" },
  denied: { label: "Denied", color: "warning" },
  failed: { label: "Failed", color: "error" },
  unknown: { label: "Unknown", color: "warning" },
} as const;

export const invocationStatus = (outcome: Invocation["outcome"]) => invocationOutcomes[outcome];

const sourceNames: Record<string, string> = {
  console: "Console", cli: "CLI", mcp: "MCP", resource_api: "API", local: "Local",
};
export const sourceName = (surface: string) =>
  Object.hasOwn(sourceNames, surface) ? sourceNames[surface] : surface;

const stageNames: Record<string, string> = {
  authentication: "Authentication",
  authorization: "Authorization",
  reauthentication: "Reauthentication",
  validation: "Validation",
  execution: "Execution",
  commit: "Commit confirmation",
  browser_admission: "Browser admission",
};
export const stageName = (stage?: string | null) =>
  stage ? (Object.hasOwn(stageNames, stage) ? stageNames[stage] : stage) : "Not recorded";

const authMethods: Record<string, string> = {
  password: "Password", session: "Browser session", api_token: "API token",
};
export const authMethodName = (method?: string | null) =>
  method ? (Object.hasOwn(authMethods, method) ? authMethods[method] : method) : "Not recorded";

export const targetName = (event: Invocation) =>
  event.target_type && event.target_id ? `${event.target_type}: ${event.target_id}` : "Not recorded";

export function recordingStatus(sampled?: boolean | null) {
  if (sampled === true) return {
    label: "Representative", color: "warning" as const,
    description: "Rate-limited anonymous record; not an attempt count.",
  };
  if (sampled === false) return {
    label: "Individual", color: "default" as const,
    description: "Individual event. Delivery is still best effort.",
  };
  return {
    label: "Unknown", color: "default" as const,
    description: "Recording mode was not recorded.",
  };
}
