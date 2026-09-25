import { useState } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, RefreshCw } from "lucide-react";
import { identityPage } from "../../api/identity";
import { ApiError, message } from "../../api/http";
import { Dialog } from "../../design/Dialog";
import {
  Event,
  Stream,
  actor,
  eventName,
  eventResult,
  validator,
} from "./model";
import { time } from "../../design/format";

export function Activity({ route, admin }: { route: string; admin: boolean }) {
  const stream: Stream =
    route === "activity/invocations"
      ? "invocations"
      : route === "activity/security"
        ? "security"
        : "audit";
  const denied = stream === "security" && !admin;
  const [selected, setSelected] = useState<Event | null>(null);
  const cache = useQueryClient();
  const query = useInfiniteQuery({
    queryKey: ["activity", stream, admin],
    enabled: !denied,
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam, signal }) => {
      try {
        return await identityPage(
          `/history/${stream}`,
          validator(stream),
          pageParam,
          signal,
        );
      } catch (error) {
        if (error instanceof ApiError && error.status === 403)
          void cache.invalidateQueries({ queryKey: ["session"] });
        throw error;
      }
    },
    getNextPageParam: (page) => page.next_before,
    gcTime: 0,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <main className="overview activity">
      <div className="page-heading">
        <div>
          <p className="eyebrow">{admin ? "INSTALLATION" : "MY ACTIVITY"}</p>
          <h1>Activity</h1>
        </div>
        <button
          className="icon-button"
          aria-label="Refresh activity"
          title="Refresh activity"
          disabled={denied || query.isFetching}
          onClick={() => {
            setSelected(null);
            void query.refetch();
          }}
        >
          <RefreshCw size={18} />
        </button>
      </div>
      <nav className="view-tabs" aria-label="Activity views">
        <a
          href="#activity"
          aria-current={stream === "audit" ? "page" : undefined}
        >
          Audit log
        </a>
        <a
          href="#activity/invocations"
          aria-current={stream === "invocations" ? "page" : undefined}
        >
          Command history
        </a>
        {admin && (
          <a
            href="#activity/security"
            aria-current={stream === "security" ? "page" : undefined}
          >
            Security events
          </a>
        )}
      </nav>
      {denied ? (
        <p role="alert">Admin access required.</p>
      ) : query.isPending ? (
        <p role="status">Loading activity...</p>
      ) : query.isError ? (
        <p role="alert">{message(query.error)}</p>
      ) : (
        <>
          <div className="event-list">
            {rows.map((event) => (
              <button
                className="event-row"
                key={event.context.id}
                onClick={() => setSelected(event)}
              >
                <time dateTime={event.context.created_at}>
                  {time(event.context.created_at)}
                </time>
                <span>
                  <strong>{eventName(event)}</strong>
                  <span className="muted">{eventResult(event)}</span>
                </span>
                <span>
                  <span>{actor(event.context)}</span>
                  <span className="muted">{event.context.surface}</span>
                </span>
                <ChevronRight size={16} />
              </button>
            ))}
          </div>
          {!rows.length && <p className="empty">No activity recorded.</p>}
          {query.hasNextPage && (
            <button
              disabled={query.isFetching}
              onClick={() => void query.fetchNextPage()}
            >
              {query.isFetching ? "Loading..." : "Load more"}
            </button>
          )}
        </>
      )}
      {!denied && selected && !query.isError && (
        <Dialog
          title="Event details"
          busy={false}
          onClose={() => setSelected(null)}
        >
          <dl className="detail-fields event-details">
            {Object.entries({
              Event: eventName(selected),
              Result: eventResult(selected),
              Time: time(selected.context.created_at),
              Actor: actor(selected.context),
              "Actor type":
                selected.context.actor_kind === "master"
                  ? "root"
                  : selected.context.actor_kind,
              Surface: selected.context.surface,
              "Token ID": selected.context.credential_id,
              "Session ID": selected.context.session_id,
              "Request ID": selected.context.request_id,
              ...("duration_ms" in selected
                ? {
                    "Duration (ms)": selected.duration_ms,
                    Error: selected.error_code,
                  }
                : {}),
            }).map(([label, value]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd>{value ?? "-"}</dd>
              </div>
            ))}
          </dl>
          {"metadata" in selected && (
            <pre className="event-metadata">
              {JSON.stringify(selected.metadata, null, 2)}
            </pre>
          )}
        </Dialog>
      )}
    </main>
  );
}
