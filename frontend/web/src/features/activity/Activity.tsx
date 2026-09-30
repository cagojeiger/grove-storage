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
  Alert,
  Box,
  Container,
  Grid,
  Stack,
  Tooltip,
  Dialog,
  DialogTitle,
  DialogActions,
} from "@mui/material";

import { useState, useId } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { ListFilter, RefreshCw } from "lucide-react";
import { field, identityPage } from "../../api/identity";
import { ApiError, message } from "../../api/http";

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
  const titleId = useId();

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
    <Container component="main" maxWidth="lg" sx={{ py: 3 }}>
      <Stack spacing={3}>
        <Stack
          direction="row"
          sx={{ alignItems: "center", justifyContent: "space-between" }}
        >
          <Typography component="h1" variant="h5">
            {admin ? "Activity" : "My activity"}
          </Typography>
          <Tooltip title="Refresh activity">
            <span>
              <IconButton
                aria-label="Refresh activity"
                disabled={denied || !valid || query.isFetching}
                onClick={() => {
                  setSelected(null);
                  void query.refetch();
                }}
              >
                <RefreshCw size={18} />
              </IconButton>
            </span>
          </Tooltip>
        </Stack>
        <Tabs
          value={denied ? false : stream}
          variant={compact ? "fullWidth" : "scrollable"}
          scrollButtons="auto"
          aria-label="Activity views"
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
            <Stack direction="row" spacing={2} sx={{ alignItems: "center" }}>
              <Button
                type="button"
                aria-expanded={filtersOpen}
                startIcon={<ListFilter size={16} />}
                aria-controls="activity-filters"
                onClick={() => setFiltersOpen(!filtersOpen)}
              >
                Filters{params.size > 0 ? ` (${params.size})` : ""}
              </Button>
              {params.size > 0 && <Link href={`#${path}`}>Clear filters</Link>}
            </Stack>
          )}
        {!denied && (
          <Stack
            component="form"
            direction={{ xs: "column", sm: "row" }}
            spacing={2}
            id="activity-filters"
            hidden={!filtersOpen}
            sx={{
              display: filtersOpen ? "flex" : "none",
              alignItems: { sm: "center" },
            }}
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
          </Stack>
        )}
        {denied ? (
          <Alert severity="error">Admin access required.</Alert>
        ) : !valid ? (
          <Alert severity="error">Enter a valid account or token ID.</Alert>
        ) : query.isPending ? (
          <Typography role="status">Loading activity...</Typography>
        ) : query.isError ? (
          <Alert severity="error">{message(query.error)}</Alert>
        ) : (
          <>
            <TableContainer>
              <Table
                size="small"
                aria-label="Activity"
                sx={{
                  tableLayout: compact ? "fixed" : "auto",
                  overflowWrap: "anywhere",
                }}
              >
                <TableHead>
                  <TableRow>
                    <TableCell sx={{ width: compact ? 104 : "auto" }}>
                      Time
                    </TableCell>
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
                      <TableCell
                        sx={{
                          whiteSpace: compact ? "normal" : "nowrap",
                          verticalAlign: "top",
                        }}
                      >
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
                            <Typography
                              variant="body2"
                              color="text.secondary"
                              sx={{ mt: 0.5 }}
                            >
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
              <Typography color="text.secondary">
                {params.size > 0
                  ? "No activity matches these filters."
                  : "No activity recorded yet."}
              </Typography>
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
            open
            fullWidth
            maxWidth="sm"
            fullScreen={compact}
            aria-labelledby={titleId}
            onClose={(_event, reason) => {
              if (reason === "escapeKeyDown") setSelected(null);
            }}
          >
            <DialogTitle id={titleId}>{"Event details"}</DialogTitle>
            <DialogContent>
              <Grid container component="dl" spacing={3}>
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
                  <Grid key={label} size={{ xs: 12, sm: 6 }}>
                    <Typography
                      component="dt"
                      variant="body2"
                      color="text.secondary"
                    >
                      {label}
                    </Typography>
                    <Typography
                      component="dd"
                      sx={{ m: 0, overflowWrap: "anywhere" }}
                    >
                      {value ?? "-"}
                    </Typography>
                  </Grid>
                ))}
              </Grid>
              {"metadata" in selected && (
                <Box
                  component="pre"
                  sx={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}
                >
                  {JSON.stringify(selected.metadata, null, 2)}
                </Box>
              )}
            </DialogContent>
            <DialogActions>
              <Button onClick={() => setSelected(null)}>Close</Button>
            </DialogActions>
          </Dialog>
        )}
      </Stack>
    </Container>
  );
}
