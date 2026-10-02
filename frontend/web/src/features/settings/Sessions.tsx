import {
  DialogContent,
  DialogActions,
  TextField,
  IconButton,
  Button,
  Alert,
  Divider,
  Link,
  List,
  ListItem,
  ListItemText,
  Stack,
  Tooltip,
  Typography,
  Dialog,
  DialogTitle,
  useMediaQuery,
  useTheme,
} from "@mui/material";

import { useState, useId } from "react";
import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { LogOut, Pencil, RefreshCw } from "lucide-react";
import { ApiError, Session, message } from "../../api/http";
import {
  Account,
  identityPage,
  identityRequest,
  isAccount,
  isChanged,
  isObject,
} from "../../api/identity";
import { clearSession } from "../../auth/session";

import { useAction } from "../access/useAction";
import { time } from "../../design/format";
import { PersonalTokens } from "./PersonalTokens";
import { Details } from "../../app/Details";
import { Page } from "../../app/Page";
import { DetailSections } from "../../app/DetailSections";
import { useRoute } from "../../app/navigation";

type LoginSession = {
  id: string;
  credential_id: string | null;
  created_at: string;
  expires_at: string;
  revoked_at: string | null;
};
function isSession(v: unknown): v is LoginSession {
  return (
    isObject(v) &&
    typeof v.id === "string" &&
    (v.credential_id === null || typeof v.credential_id === "string") &&
    ["created_at", "expires_at"].every(
      (k) => typeof v[k] === "string" && Number.isFinite(Date.parse(v[k])),
    ) &&
    (v.revoked_at === null || typeof v.revoked_at === "string")
  );
}
export function Sessions({ session }: { session: Session }) {
  const route = useRoute();
  const sessionsSelected =
    new URLSearchParams(route.split("?")[1]).get("tab") === "sessions";
  const cache = useQueryClient();
  const [selected, setSelected] = useState<LoginSession | null>(null);
  const [editing, setEditing] = useState(false);
  const passwordSession = session.credential_id === null;
  const profile = useQuery({
    queryKey: ["me", "profile", session.user_id],
    enabled: passwordSession,
    queryFn: ({ signal }) => identityRequest("/me", isAccount, { signal }),
    gcTime: 0,
  });
  const query = useInfiniteQuery({
    queryKey: ["sessions", session.session_id],
    enabled: sessionsSelected,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) =>
      identityPage("/me/sessions", isSession, pageParam, signal),
    getNextPageParam: (page) => page.next_before,
    gcTime: 0,
  });
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <Page
      title="My account"
      actions={
        sessionsSelected && (
          <Tooltip title="Refresh sessions">
            <span>
              <IconButton
                aria-label="Refresh sessions"
                disabled={query.isFetching}
                onClick={() => void query.refetch()}
              >
                <RefreshCw size={18} />
              </IconButton>
            </span>
          </Tooltip>
        )
      }
    >
      <DetailSections
        label="My account sections"
        sections={[
          {
            value: "profile",
            label: "Profile",
            content: (
              <Stack spacing={3}>
                {passwordSession && profile.isPending && (
                  <Typography role="status">Loading profile...</Typography>
                )}
                {passwordSession && profile.isError && (
                  <Alert severity="error">
                    {message(profile.error)}{" "}
                    <Button
                      type="submit"
                      onClick={() => void profile.refetch()}
                    >
                      Retry
                    </Button>
                  </Alert>
                )}
                {profile.data && (
                  <Stack
                    direction="row"
                    spacing={2}
                    sx={{
                      alignItems: "center",
                      justifyContent: "space-between",
                    }}
                  >
                    <Typography
                      component="h2"
                      variant="h6"
                      sx={{ overflowWrap: "anywhere", minWidth: 0 }}
                    >
                      {profile.data.display_name}
                    </Typography>
                    <Tooltip title="Edit my name">
                      <IconButton
                        aria-label="Edit my name"
                        onClick={() => setEditing(true)}
                      >
                        <Pencil size={16} />
                      </IconButton>
                    </Tooltip>
                  </Stack>
                )}
                <Details
                  label="My account details"
                  items={[
                    ...(profile.data
                      ? [
                          ["Username", profile.data.username] as [
                            string,
                            string | null,
                          ],
                        ]
                      : []),
                    ["Account", session.user_id],
                    ["Role", session.role],
                  ]}
                />
                {editing && profile.data && (
                  <EditProfile
                    account={profile.data}
                    onClose={() => setEditing(false)}
                    onSaved={async () => {
                      await cache.invalidateQueries({
                        queryKey: ["me", "profile"],
                      });
                    }}
                  />
                )}
                {passwordSession && (
                  <Stack component="section" aria-label="Security" spacing={2}>
                    <Divider />
                    <Typography component="h2" variant="h6">
                      Security
                    </Typography>
                    <Link href="#settings/security">Change password</Link>
                  </Stack>
                )}
              </Stack>
            ),
          },
          ...(passwordSession
            ? [
                {
                  value: "tokens",
                  label: "API tokens",
                  content: <PersonalTokens session={session} />,
                },
              ]
            : []),
          {
            value: "sessions",
            label: "Sessions",
            content: (
              <Stack component="section" aria-label="My sessions" spacing={2}>
                <Typography component="h2" variant="h6">
                  My sessions
                </Typography>
                {query.isPending ? (
                  <Typography role="status">Loading sessions...</Typography>
                ) : query.isError ? (
                  <Alert severity="error">{message(query.error)}</Alert>
                ) : (
                  <>
                    <List disablePadding aria-label="My sessions">
                      {rows.map((row) => {
                        const current = row.id === session.session_id;
                        const state = row.revoked_at
                          ? "Revoked"
                          : Date.parse(row.expires_at) <= Date.now()
                            ? "Expired"
                            : "Active";
                        return (
                          <ListItem
                            key={row.id}
                            divider
                            disableGutters
                            secondaryAction={
                              <Tooltip
                                title={
                                  current
                                    ? "Revoke current session"
                                    : `Revoke session ${row.id}`
                                }
                              >
                                <span>
                                  <IconButton
                                    edge="end"
                                    color="error"
                                    aria-label={
                                      current
                                        ? "Revoke current session"
                                        : `Revoke session ${row.id}`
                                    }
                                    disabled={state !== "Active"}
                                    onClick={() => setSelected(row)}
                                  >
                                    <LogOut size={17} />
                                  </IconButton>
                                </span>
                              </Tooltip>
                            }
                          >
                            <ListItemText
                              primary={
                                current ? "Current session" : "Browser session"
                              }
                              sx={{ overflowWrap: "anywhere" }}
                              slotProps={{ secondary: { component: "div" } }}
                              secondary={
                                <Stack spacing={0.5}>
                                  <Typography
                                    component="code"
                                    variant="body2"
                                    sx={{ fontFamily: "monospace" }}
                                  >
                                    {row.id}
                                  </Typography>
                                  <Typography variant="body2">
                                    {row.credential_id
                                      ? `Token: ${row.credential_id}`
                                      : "Password sign-in"}
                                  </Typography>
                                  <Typography variant="body2">
                                    {state}
                                  </Typography>
                                  <Typography variant="body2">
                                    Created {time(row.created_at)}
                                  </Typography>
                                  <Typography variant="body2">
                                    Expires {time(row.expires_at)}
                                  </Typography>
                                </Stack>
                              }
                            />
                          </ListItem>
                        );
                      })}
                    </List>
                    {!rows.length && (
                      <Typography color="text.secondary">
                        No sessions found.
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
              </Stack>
            ),
          },
        ]}
      />
      {selected && (
        <RevokeSession
          row={selected}
          current={selected.id === session.session_id}
          onClose={() => setSelected(null)}
          onRevoked={async () => {
            if (selected.id === session.session_id) clearSession(cache);
            else {
              setSelected(null);
              await cache.invalidateQueries({ queryKey: ["sessions"] });
            }
          }}
        />
      )}
    </Page>
  );
}

function EditProfile({
  account,
  onClose,
  onSaved,
}: {
  account: Account;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));

  const action = useAction();
  const [name, setName] = useState(account.display_name);
  return (
    <Dialog
      open
      fullWidth
      maxWidth="sm"
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !action.busy) onClose();
      }}
    >
      <DialogTitle id={titleId}>{"Edit my name"}</DialogTitle>
      <>
        <DialogContent dividers>
          <form
            id={`${titleId}-form`}
            onSubmit={(event) => {
              event.preventDefault();
              void action.run(async () => {
                await identityRequest("/me", isChanged, {
                  method: "PATCH",
                  body: JSON.stringify({ display_name: name.trim() }),
                });
                onClose();
                await onSaved();
              });
            }}
          >
            <Stack spacing={3}>
              <TextField
                value={name}
                onChange={(event) => setName(event.target.value)}
                required
                label={"Display name"}
                disabled={action.busy || action.unknown}
                slotProps={{ htmlInput: { maxLength: 80 } }}
              />
              {action.error && <Alert severity="error">{action.error}</Alert>}
            </Stack>
          </form>
        </DialogContent>
        <DialogActions>
          <Button type="button" disabled={action.busy} onClick={onClose}>
            {action.unknown ? "Close and review" : "Cancel"}
          </Button>
          <Button
            type="submit"
            form={`${titleId}-form`}
            variant="contained"
            disabled={
              action.busy ||
              action.unknown ||
              !name.trim() ||
              name.trim() === account.display_name
            }
          >
            Save
          </Button>
        </DialogActions>
      </>
    </Dialog>
  );
}
function RevokeSession({
  row,
  current,
  onClose,
  onRevoked,
}: {
  row: LoginSession;
  current: boolean;
  onClose: () => void;
  onRevoked: () => Promise<void>;
}) {
  const titleId = useId();
  const fullScreen = useMediaQuery(useTheme().breakpoints.down("sm"));

  const action = useAction((error) =>
    error instanceof ApiError && error.outcome === "not_applied"
      ? message(error)
      : "The outcome is unknown. Close and refresh the session list before trying again.",
  );
  return (
    <Dialog
      open
      fullWidth
      maxWidth="sm"
      fullScreen={fullScreen}
      aria-labelledby={titleId}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !action.busy) onClose();
      }}
    >
      <DialogTitle id={titleId}>{"Revoke session"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          <Typography>
            {current
              ? "This signs you out of the current console session."
              : "This ends the selected browser session. The account token remains valid."}
          </Typography>
          <Typography
            component="code"
            variant="body2"
            sx={{ fontFamily: "monospace", overflowWrap: "anywhere" }}
          >
            {row.id}
          </Typography>
          {action.error && <Alert severity="error">{action.error}</Alert>}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button type="button" disabled={action.busy} onClick={onClose}>
          {action.unknown ? "Close and review" : "Cancel"}
        </Button>
        <Button
          type="button"
          variant="contained"
          color="error"
          disabled={action.busy || action.unknown}
          onClick={() =>
            void action.run(async () => {
              await identityRequest(
                `/me/sessions/${encodeURIComponent(row.id)}`,
                isChanged,
                { method: "DELETE" },
              );
              await onRevoked();
            })
          }
        >
          {action.busy ? "Revoking..." : "Confirm revoke"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
