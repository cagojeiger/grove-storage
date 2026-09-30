import { IconButton, Button, ButtonBase, Tabs, Tab } from "@mui/material";
import { Input } from "../../design/Fields";
import { useState } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronRight, ListFilter, RefreshCw } from "lucide-react";
import { field, identityPage } from "../../api/identity";
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
import { activityFilters, uuidPattern } from "./filters";

export function Activity({ route, admin }: { route: string; admin: boolean }) {
  const path = route.split("?")[0];
  const { params, valid } = activityFilters(route);
  const suffix = params.size ? `?${params}` : "";
  const stream: Stream =
    path === "activity/invocations"
      ? "invocations"
      : path === "activity/security"
        ? "security"
        : "audit";
  const denied = stream === "security" && !admin;
  const [selected, setSelected] = useState<Event | null>(null);
  const [filtersOpen, setFiltersOpen] = useState(params.size > 0 || !valid);
  const cache = useQueryClient();
  const query = useInfiniteQuery({
    queryKey: ["activity", stream, admin, params.toString()],
    enabled: !denied && valid,
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam, signal }) => {
      try {
        return await identityPage(
          `/history/${stream}`,
          validator(stream),
          pageParam,
          signal,
          params,
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
        <IconButton type="submit"
          className="icon-button"
          aria-label="Refresh activity"
          title="Refresh activity"
          disabled={denied || !valid || query.isFetching}
          onClick={() => {
            setSelected(null);
            void query.refetch();
          }}
        >
          <RefreshCw size={18} />
        </IconButton>
      </div>
      <Tabs value={denied ? false : stream} variant="scrollable" scrollButtons="auto" aria-label="Activity views">
        <Tab component="a" value="audit" href={`#activity${suffix}`} label="Audit log" />
        <Tab component="a" value="invocations" href={`#activity/invocations${suffix}`} label="Command history" />
        {admin && <Tab component="a" value="security" href={`#activity/security${suffix}`} label="Security events" />}
      </Tabs>
      {!denied && (rows.length > 0 || params.size > 0 || query.isPending || query.isError) &&
        <div className="activity-toolbar">
          <Button type="button" aria-expanded={filtersOpen} aria-controls="activity-filters" onClick={() => setFiltersOpen(!filtersOpen)}>
            <ListFilter size={16} />Filters{params.size > 0 ? ` (${params.size})` : ""}
          </Button>
          {params.size > 0 && <a href={`#${path}`}>Clear filters</a>}
        </div>}
      {!denied && (
        <form id="activity-filters" className="activity-filters" hidden={!filtersOpen} onSubmit={(event) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const next = new URLSearchParams();
          for (const key of ["account_id", "credential_id"]) {
            const value = field(data, key).trim();
            if (value) next.set(key, value);
          }
          window.location.hash = `${path}${next.size ? `?${next}` : ""}`;
        }}>
          <label>Actor account ID
            <Input name="account_id" defaultValue={params.get("account_id") ?? ""} pattern={uuidPattern} autoComplete="off" spellCheck={false} />
          </label>
          <label>Used token ID
            <Input name="credential_id" defaultValue={params.get("credential_id") ?? ""} pattern={uuidPattern} autoComplete="off" spellCheck={false} />
          </label>
          <Button type="submit">Apply</Button>
        </form>
      )}
      {denied ? (
        <p role="alert">Admin access required.</p>
      ) : !valid ? (
        <p role="alert">Enter a valid account or token ID.</p>
      ) : query.isPending ? (
        <p role="status">Loading activity...</p>
      ) : query.isError ? (
        <p role="alert">{message(query.error)}</p>
      ) : (
        <>
          <div className="event-list">
            {rows.map((event) => (
              <ButtonBase type="submit"
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
              </ButtonBase>
            ))}
          </div>
          {!rows.length && <p className="empty">{params.size > 0 ? "No activity matches these filters." : "No activity recorded yet."}</p>}
          {query.hasNextPage && (
            <Button type="submit"
              disabled={query.isFetching}
              onClick={() => void query.fetchNextPage()}
            >
              {query.isFetching ? "Loading..." : "Load more"}
            </Button>
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
