import { Button } from "@mui/material";
import { useState } from "react";
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
import { ConsoleLayout } from "./ConsoleLayout";
import { Overview } from "../features/overview/Overview";
import { Storages } from "../features/storages/Storages";
import { clearSession } from "../auth/session";
import { useRoute } from "./navigation";
import { Access } from "../features/access/Access";
import { Clients } from "../features/clients/Clients";
import { Activity } from "../features/activity/Activity";
import { Sessions } from "../features/settings/Sessions";
import { Security } from "../features/settings/Security";
import { UsageHistory } from "../features/overview/UsageHistory";

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
    <ConsoleLayout
      session={route === "set-password" ? null : session.data}
      route={route}
      name={profile.data?.display_name ?? "Account"}
      loggingOut={loggingOut}
      onLogout={() => void logout()}
    >
      {route === "set-password" ? (
        <SetPassword key={fullRoute} />
      ) : session.isPending ? (
        <main className="connection" role="status">
          Checking session...
        </main>
      ) : session.isError ? (
        <main className="connection">
          <p role="alert">{message(session.error)}</p>
          <Button type="submit" onClick={() => void session.refetch()}>
            Reconnect
          </Button>
        </main>
      ) : session.data ? (
        <>
          {logoutError && (
            <p className="logout-error" role="alert">
              {logoutError}
            </p>
          )}
          {accessPage ? (
            session.data.role === "admin" ? (
              <Access
                key={`${route}:${session.data.session_id}`}
                route={route}
                currentUserId={session.data.user_id}
              />
            ) : (
              <main className="connection">
                <p role="alert">Admin access required.</p>
              </main>
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
                <main className="connection">
                  <p role="alert">Password sign-in required.</p>
                </main>
              )
            ) : (
              <Sessions key={session.data.session_id} session={session.data} />
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
        </>
      ) : (
        <Login
          onLogin={(value) => {
            setLogoutError("");
            cache.setQueryData(["session"], value);
          }}
        />
      )}
    </ConsoleLayout>
  );
}
