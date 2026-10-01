import { Alert, Button, Container, Stack, Typography } from "@mui/material";
import { lazy, Suspense, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  identity,
  ApiError,
  currentSession,
  message,
  request,
} from "../api/http";
import { identityRequest, isAccount } from "../api/identity";
import { Login } from "../auth/Login";
import { SetPassword } from "../auth/SetPassword";
import { Shell } from "./Shell";
import { Overview } from "../features/overview/Overview";
import { clearSession } from "../auth/session";
import { useRoute } from "./navigation";
const Access = lazy(() =>
  import("../features/access/Access").then((module) => ({
    default: module.Access,
  })),
);
const Activity = lazy(() =>
  import("../features/activity/Activity").then((module) => ({
    default: module.Activity,
  })),
);
const Sessions = lazy(() =>
  import("../features/settings/Sessions").then((module) => ({
    default: module.Sessions,
  })),
);
const Security = lazy(() =>
  import("../features/settings/Security").then((module) => ({
    default: module.Security,
  })),
);
const UsageHistory = lazy(() =>
  import("../features/overview/UsageHistory").then((module) => ({
    default: module.UsageHistory,
  })),
);

const Storages = lazy(() =>
  import("../features/storages/Storages").then((module) => ({
    default: module.Storages,
  })),
);
const Clients = lazy(() =>
  import("../features/clients/Clients").then((module) => ({
    default: module.Clients,
  })),
);

export function App() {
  const cache = useQueryClient();
  const fullRoute = useRoute();
  const route = fullRoute.split("?")[0];
  const storagePage = route === "storages" || route.startsWith("storages/");
  const accessPage =
    route === "accounts" ||
    route.startsWith("accounts/") ||
    route === "access" ||
    route.startsWith("access/");
  const clientPage = route === "clients" || route.startsWith("clients/");
  const activityPage = route === "activity" || route.startsWith("activity/");
  const settingsPage = route === "settings" || route === "settings/security";
  const [loggingOut, setLoggingOut] = useState(false);
  const [logoutError, setLogoutError] = useState("");
  const session = useQuery({
    queryKey: ["session"],
    queryFn: async ({ signal }) => {
      try {
        return await currentSession(signal);
      } catch (error) {
        if (error instanceof ApiError && error.status === 401) {
          cache.removeQueries({
            predicate: (query) => query.queryKey[0] !== "session",
          });
          return null;
        }
        throw error;
      }
    },
    retry: false,
  });
  const profile = useQuery({
    queryKey: ["me", "profile", session.data?.user_id],
    enabled: Boolean(session.data && session.data.credential_id === null),
    queryFn: ({ signal }) => identityRequest("/me", isAccount, { signal }),
  });
  async function logout() {
    setLoggingOut(true);
    setLogoutError("");
    try {
      await request(`${identity}/session`, { method: "DELETE" });
      clearSession(cache);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        clearSession(cache);
        return;
      }
      setLogoutError(message(error));
    } finally {
      setLoggingOut(false);
    }
  }
  return (
    <Shell
      session={route === "set-password" ? null : session.data}
      route={route}
      name={profile.data?.display_name ?? "Account"}
      loggingOut={loggingOut}
      onLogout={() => void logout()}
    >
      {route === "set-password" ? (
        <SetPassword key={fullRoute} />
      ) : session.isPending ? (
        <Container component="main" maxWidth="sm" sx={{ py: 6 }}>
          <Typography role="status">Checking session...</Typography>
        </Container>
      ) : session.isError ? (
        <Container component="main" maxWidth="sm" sx={{ py: 6 }}>
          <Stack spacing={2}>
            <Alert severity="error">{message(session.error)}</Alert>
            <Button type="submit" onClick={() => void session.refetch()}>
              Reconnect
            </Button>
          </Stack>
        </Container>
      ) : session.data ? (
        <>
          {logoutError && <Alert severity="error">{logoutError}</Alert>}
          <Suspense
            fallback={
              <Container component="main" sx={{ py: 3 }}>
                <Typography role="status">Loading page...</Typography>
              </Container>
            }
          >
            {accessPage ? (
              session.data.role === "admin" ? (
                <Access
                  key={`${route}:${session.data.session_id}`}
                  route={route}
                  currentUserId={session.data.user_id}
                />
              ) : (
                <Container component="main" maxWidth="sm" sx={{ py: 6 }}>
                  <Alert severity="error">Admin access required.</Alert>
                </Container>
              )
            ) : activityPage ? (
              <Activity
                key={`${fullRoute}:${session.data.role}:${session.data.session_id}`}
                route={fullRoute}
                admin={session.data.role === "admin"}
              />
            ) : settingsPage ? (
              route === "settings/security" ? (
                session.data.credential_id === null ? (
                  <Security />
                ) : (
                  <Container component="main" maxWidth="sm" sx={{ py: 6 }}>
                    <Alert severity="error">Password sign-in required.</Alert>
                  </Container>
                )
              ) : (
                <Sessions
                  key={session.data.session_id}
                  session={session.data}
                />
              )
            ) : clientPage ? (
              <Clients
                key={`${route}:${session.data.role}`}
                route={route}
                canWrite={session.data.role !== "reader"}
              />
            ) : storagePage ? (
              <Storages
                key={`${route}:${session.data.role}`}
                route={route}
                canWrite={session.data.role !== "reader"}
              />
            ) : route === "usage" ? (
              <UsageHistory />
            ) : (
              <Overview />
            )}
          </Suspense>
        </>
      ) : (
        <Login
          onLogin={(value) => {
            setLogoutError("");
            cache.setQueryData(["session"], value);
          }}
        />
      )}
    </Shell>
  );
}
