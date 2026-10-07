import { useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Alert,
  Button,
  Divider,
  IconButton,
  InputAdornment,
  List,
  ListItem,
  ListItemText,
  Stack,
  Tab,
  Tabs,
  Tooltip,
  Typography,
} from "@mui/material";
import { ChevronDown, Copy } from "lucide-react";
import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { identity, request, type Session } from "../api/http";
import {
  field,
  identityPage,
  identityRequest,
  isAccount,
  isChanged,
} from "../api/identity";
import { clearSession } from "../auth/session";
import { useRoute } from "../app/navigation";
import { useAction } from "../hooks/useAction";
import { time } from "../design/format";
import {
  isBrowserSession,
  reauthenticationError,
  type BrowserSession,
} from "./identityModel";
import {
  Confirmation,
  Page,
  Properties,
  QueryState,
  Refresh,
} from "./ui";
import { Tokens } from "./Tokens";

export function Profile({
  session,
  security = false,
}: {
  session: Session;
  security?: boolean;
}) {
  const route = useRoute();
  const cache = useQueryClient();
  const params = new URLSearchParams(route.split("?")[1]);
  const tab = security
    ? "security"
    : params.get("tab") === "tokens"
      ? "tokens"
      : params.get("tab") === "sessions"
        ? "sessions"
        : "profile";
  const password = session.credential_id === null;
  const profile = useQuery({
    queryKey: ["me", "profile", session.user_id],
    enabled: password,
    queryFn: ({ signal }) => identityRequest("/me", isAccount, { signal }),
  });
  const sessions = useInfiniteQuery({
    queryKey: ["sessions", session.session_id],
    enabled: tab === "sessions",
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) =>
      identityPage("/me/sessions", isBrowserSession, pageParam, signal),
    getNextPageParam: (page) => page.next_before,
  });
  const [selected, setSelected] = useState<BrowserSession | null>(null);
  return (
    <Page title="My account">
      <Tabs
        value={!password && (tab === "security" || tab === "tokens") ? false : tab}
        variant="scrollable"
        scrollButtons="auto"
        aria-label="My account sections"
      >
        <Tab component="a" href="#settings" value="profile" label="Profile" />
        {password && (
          <Tab
            component="a"
            href="#settings/security"
            value="security"
            label="Security"
          />
        )}
        {password && (
          <Tab
            component="a"
            href="#settings?tab=tokens"
            value="tokens"
            label="API tokens"
          />
        )}
        <Tab
          component="a"
          href="#settings?tab=sessions"
          value="sessions"
          label="Sessions"
        />
      </Tabs>
      {tab === "profile" && (
        <>
          <QueryState
            pending={password && profile.isPending}
            error={profile.error}
            retry={() => void profile.refetch()}
          />
          {password && profile.data && !profile.isError && (
            <ProfileForm
              key={`${session.user_id}:${profile.data.display_name}`}
              name={profile.data.display_name}
              username={profile.data.username ?? ""}
              role={session.role}
            />
          )}
          {!password && <Properties values={{ Role: session.role }} />}
          <AccountDetails id={session.user_id} />
        </>
      )}
      {tab === "security" &&
        (password ? (
          <Password />
        ) : (
          <Alert severity="error">Password sign-in required.</Alert>
        ))}
      {tab === "tokens" &&
        (password ? (
          <Tokens accountId={session.user_id} personal />
        ) : (
          <Alert severity="error">Password sign-in required.</Alert>
        ))}
      {tab === "sessions" && (
        <Stack spacing={2}>
          <Stack direction="row" spacing={2} sx={{ alignItems: "center", justifyContent: "space-between" }}>
            <Typography component="h2" variant="h6">
              My sessions
            </Typography>
            <Refresh
              label="Refresh sessions"
              disabled={sessions.isFetching}
              onClick={() => {
                setSelected(null);
                void sessions.refetch();
              }}
            />
          </Stack>
          <QueryState
            pending={sessions.isPending}
            error={sessions.error}
            retry={() => void sessions.refetch()}
          />
          {!sessions.isError && (
            <List disablePadding aria-label="My sessions">
              {sessions.data?.pages
                .flatMap((page) => page.items)
                .map((row) => (
                  <ListItem divider disableGutters key={row.id} sx={{ gap: 2 }}>
                    <ListItemText
                      primary={
                        row.id === session.session_id
                          ? "Current session"
                          : "Browser session"
                      }
                      secondary={`${row.credential_id ? "Token sign-in" : "Password sign-in"} · Expires ${time(row.expires_at)}`}
                    />
                    <Button
                      color="error"
                      disabled={
                        Boolean(row.revoked_at) ||
                        Date.parse(row.expires_at) <= Date.now()
                      }
                      aria-label={
                        row.id === session.session_id
                          ? "Revoke current session"
                          : `Revoke session ${row.id}`
                      }
                      onClick={() => setSelected(row)}
                    >
                      Revoke
                    </Button>
                  </ListItem>
                ))}
            </List>
          )}
          {sessions.hasNextPage && (
            <Button
              disabled={sessions.isFetching}
              onClick={() => void sessions.fetchNextPage()}
            >
              Load more
            </Button>
          )}
        </Stack>
      )}
      {selected && (
        <Confirmation
          title="Revoke session"
          text={
            selected.id === session.session_id
              ? "This signs you out of the current console session."
              : "This ends the selected browser session. The account token remains valid."
          }
          confirm="REVOKE"
          onClose={() => setSelected(null)}
          execute={async () => {
            await identityRequest(
              `/me/sessions/${encodeURIComponent(selected.id)}`,
              isChanged,
              { method: "DELETE" },
            );
            if (selected.id === session.session_id) clearSession(cache);
            else {
              setSelected(null);
              await sessions.refetch();
            }
          }}
        />
      )}
    </Page>
  );
}
function ProfileForm({ name, username, role }: { name: string; username: string; role: string }) {
  const cache = useQueryClient();
  const action = useAction();
  const [draft, setDraft] = useState(name);
  const value = draft.trim();
  const valid = value.length > 0 && Array.from(value).length <= 80;
  return (
    <Stack
      component="form"
      spacing={3}
      sx={{ maxWidth: 560 }}
      onSubmit={(event) => {
        event.preventDefault();
        if (!valid || value === name) return;
        void action.run(async () => {
          await identityRequest("/me", isChanged, {
            method: "PATCH",
            body: JSON.stringify({ display_name: value }),
          });
          await cache.invalidateQueries({ queryKey: ["me"] });
        });
      }}
    >
      <TextField
        fullWidth
        label="Display name"
        name="display_name"
        autoComplete="name"
        required
        value={draft}
        disabled={action.busy || action.unknown}
        onChange={(event) => setDraft(event.target.value)}
        slotProps={{ htmlInput: { maxLength: 80 } }}
      />
      <Properties values={{ Username: username, Role: role }} />
      {action.error && <Alert severity="error">{action.error}</Alert>}
      <Button
        type="submit"
        variant="contained"
        sx={{ alignSelf: "flex-start" }}
        disabled={!valid || value === name || action.busy || action.unknown}
      >
        Save changes
      </Button>
    </Stack>
  );
}

function AccountDetails({ id }: { id: string }) {
  const [message, setMessage] = useState("");
  return (
    <Accordion disableGutters elevation={0} sx={{ maxWidth: 560 }}>
      <AccordionSummary expandIcon={<ChevronDown size={20} />}>
        <Typography>Technical details</Typography>
      </AccordionSummary>
      <AccordionDetails>
        <TextField
          fullWidth
          label="Account ID"
          value={id}
          slotProps={{ input: {
            readOnly: true,
            endAdornment: (
              <InputAdornment position="end">
                <Tooltip title="Copy account ID">
                  <IconButton aria-label="Copy account ID" onClick={() => {
                    if (!navigator.clipboard) {
                      setMessage("Copy unavailable. Select the value manually.");
                      return;
                    }
                    void navigator.clipboard.writeText(id).then(
                      () => setMessage("Account ID copied."),
                      () => setMessage("Copy failed. Select the value manually."),
                    );
                  }}>
                    <Copy size={18} />
                  </IconButton>
                </Tooltip>
              </InputAdornment>
            ),
          } }}
        />
        {message && <Typography role="status" variant="body2">{message}</Typography>}
      </AccordionDetails>
    </Accordion>
  );
}

function Password() {
  const cache = useQueryClient();
  const action = useAction(reauthenticationError, true);
  const [mismatch, setMismatch] = useState(false);
  return (
    <Stack component="section" spacing={3} sx={{ maxWidth: 560 }}>
      <Typography component="h2" variant="h6">
        Password
      </Typography>
      <Divider />
      <Stack
        component="form"
        spacing={3}
        onSubmit={(event) => {
          event.preventDefault();
          const form = event.currentTarget;
          const data = new FormData(form);
          if (field(data, "new_password") !== field(data, "confirmation")) {
            setMismatch(true);
            return;
          }
          const body = {
            current_password: field(data, "current_password"),
            new_password: field(data, "new_password"),
          };
          void action.run(async () => {
            form.reset();
            setMismatch(false);
            await request(`${identity}/me/password`, {
              method: "POST",
              body: JSON.stringify(body),
            });
            clearSession(cache);
          });
        }}
      >
        <TextField
          label="Current password"
          fullWidth
          name="current_password"
          type="password"
          autoComplete="current-password"
          required
          disabled={action.busy || action.unknown}
        />
        <TextField
          label="New password"
          fullWidth
          name="new_password"
          type="password"
          autoComplete="new-password"
          required
          disabled={action.busy || action.unknown}
          slotProps={{ htmlInput: { minLength: 15, maxLength: 128 } }}
        />
        <TextField
          label="Confirm new password"
          fullWidth
          name="confirmation"
          type="password"
          autoComplete="new-password"
          required
          disabled={action.busy || action.unknown}
          error={mismatch}
          helperText={mismatch ? "The new passwords do not match." : undefined}
          onChange={() => setMismatch(false)}
          slotProps={{ htmlInput: { minLength: 15, maxLength: 128 } }}
        />
        {action.error && <Alert severity="error">{action.error}</Alert>}
        <Button
          type="submit"
          variant="contained"
          sx={{ alignSelf: "flex-start" }}
          disabled={action.busy || action.unknown}
        >
          Change password
        </Button>
      </Stack>
    </Stack>
  );
}
