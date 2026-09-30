import {
  DialogContent,
  TextField,
  IconButton,
  Button,
  Link,
  Tabs,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { useState } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { ListFilter, RefreshCw } from "lucide-react";
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
  const compact = useMediaQuery(useTheme().breakpoints.down("sm"));
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
          <h1>{admin ? "Activity" : "My activity"}</h1>
        </div>
        <IconButton
          type="submit"
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
      <Tabs
        value={denied ? false : stream}
        variant={compact ? "fullWidth" : "scrollable"}
        scrollButtons="auto"
        aria-label="Activity views"
        sx={{ "& .MuiTab-root": { minWidth: 0 } }}
      >
        <Tab
          component="a"
          value="audit"
          href={`#activity${suffix}`}
          label="Audit log"
          wrapped={compact}
        />
        <Tab
          component="a"
          value="invocations"
          href={`#activity/invocations${suffix}`}
          label="Command history"
          wrapped={compact}
        />
        {admin && (
          <Tab
            component="a"
            value="security"
            href={`#activity/security${suffix}`}
            label="Security events"
            wrapped={compact}
          />
        )}
      </Tabs>
      {!denied &&
        (rows.length > 0 ||
          params.size > 0 ||
          query.isPending ||
          query.isError) && (
          <div className="activity-toolbar">
            <Button
              type="button"
              aria-expanded={filtersOpen}
              aria-controls="activity-filters"
              onClick={() => setFiltersOpen(!filtersOpen)}
            >
              <ListFilter size={16} />
              Filters{params.size > 0 ? ` (${params.size})` : ""}
            </Button>
            {params.size > 0 && <a href={`#${path}`}>Clear filters</a>}
          </div>
        )}
      {!denied && (
        <form
          id="activity-filters"
          className="activity-filters"
          hidden={!filtersOpen}
          onSubmit={(event) => {
            event.preventDefault();
            const data = new FormData(event.currentTarget);
            const next = new URLSearchParams();
            for (const key of ["account_id", "credential_id"]) {
              const value = field(data, key).trim();
              if (value) next.set(key, value);
            }
            window.location.hash = `${path}${next.size ? `?${next}` : ""}`;
          }}
        >
          <TextField
            name="account_id"
            defaultValue={params.get("account_id") ?? ""}
            autoComplete="off"
            label={"Actor account ID"}
            slotProps={{
              htmlInput: { pattern: uuidPattern, spellCheck: false },
            }}
          />
          <TextField
            name="credential_id"
            defaultValue={params.get("credential_id") ?? ""}
            autoComplete="off"
            label={"Used token ID"}
            slotProps={{
              htmlInput: { pattern: uuidPattern, spellCheck: false },
            }}
          />
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
          <TableContainer>
            <Table
              size="small"
              aria-label="Activity"
              sx={{ tableLayout: compact ? "fixed" : "auto", overflowWrap: "anywhere" }}
            >
              <TableHead>
                <TableRow>
                  <TableCell sx={{ width: compact ? 104 : "auto" }}>Time</TableCell>
                  <TableCell>Event</TableCell>
                  {!compact && (
                    <>
                      <TableCell>
                        {stream === "audit"
                          ? "Resource"
                          : stream === "security"
                            ? "Reason"
                            : "Result"}
                      </TableCell>
                      <TableCell>Actor / Source</TableCell>
                    </>
                  )}
                </TableRow>
              </TableHead>
              <TableBody>
                {rows.map((event) => (
                  <TableRow key={event.context.id} hover>
                    <TableCell sx={{ whiteSpace: compact ? "normal" : "nowrap", verticalAlign: "top" }}>
                      <time dateTime={event.context.created_at}>
                        {time(event.context.created_at)}
                      </time>
                    </TableCell>
                    <TableCell>
                      <Link
                        component="button"
                        onClick={() => setSelected(event)}
                        sx={{ textAlign: "left" }}
                      >
                        {eventName(event)}
                      </Link>
                      {compact && (
                        <>
                          <Typography variant="body2" sx={{ mt: 0.5 }}>
                            {eventResult(event)}
                          </Typography>
                          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
                            {actor(event.context)}
                          </Typography>
                          <Typography variant="body2" color="text.secondary">
                            {event.context.surface}
                          </Typography>
                        </>
                      )}
                    </TableCell>
                    {!compact && (
                      <>
                        <TableCell>{eventResult(event)}</TableCell>
                        <TableCell>
                          {actor(event.context)}
                          <Typography variant="body2" color="text.secondary">
                            {event.context.surface}
                          </Typography>
                        </TableCell>
                      </>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
          {!rows.length && (
            <p className="empty">
              {params.size > 0
                ? "No activity matches these filters."
                : "No activity recorded yet."}
            </p>
          )}
          {query.hasNextPage && (
            <Button
              type="submit"
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
          <DialogContent>
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
          </DialogContent>
        </Dialog>
      )}
    </main>
  );
}
