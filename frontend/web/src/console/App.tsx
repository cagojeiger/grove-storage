import { lazy, Suspense, useState } from "react";
import { Alert, Button, Container, LinearProgress } from "@mui/material";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ApiError,
  currentSession,
  identity,
  message,
  request,
} from "../api/http";
import { identityRequest, isAccount } from "../api/identity";
import { clearSession } from "../auth/session";
import { useRoute } from "../app/navigation";
import { Auth } from "./Auth";
import Layout from "../template/dashboard/Dashboard";
import { Overview } from "./Overview";
const Resources = lazy(() =>
  import("./Resources").then((module) => ({ default: module.Resources })),
);
const UsageHistory = lazy(() =>
  import("./UsageHistory").then((module) => ({ default: module.UsageHistory })),
);
const Accounts = lazy(() =>
  import("./Accounts").then((module) => ({ default: module.Accounts })),
);
const Activity = lazy(() =>
  import("./Activity").then((module) => ({ default: module.Activity })),
);
const Profile = lazy(() =>
  import("./Profile").then((module) => ({ default: module.Profile })),
);

export function App() {
  const cache = useQueryClient();
  const fullRoute = useRoute();
  const route = fullRoute.split("?")[0];
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const session = useQuery({
    queryKey: ["session"],
    queryFn: async ({ signal }) => {
      try {
        return await currentSession(signal);
      } catch (failure) {
        if (failure instanceof ApiError && failure.status === 401) {
          cache.removeQueries({
            predicate: (query) => query.queryKey[0] !== "session",
          });
          return null;
        }
        throw failure;
      }
    },
  });
  const profile = useQuery({
    queryKey: ["me", "profile", session.data?.user_id],
    enabled: Boolean(session.data && session.data.credential_id === null),
    queryFn: ({ signal }) => identityRequest("/me", isAccount, { signal }),
  });
  if (route === "set-password") return <Auth key={fullRoute} setup />;
  if (session.isPending)
    return <LinearProgress aria-label="Checking session" />;
  if (session.isError)
    return (
      <Container sx={{ py: 4 }}>
        <Alert
          severity="error"
          action={
            <Button onClick={() => void session.refetch()}>Reconnect</Button>
          }
        >
          {message(session.error)}
        </Alert>
      </Container>
    );
  if (!session.data) return <Auth />;
  const value = session.data;
  const storageEditor =
    route === "storages/create/new" || /^storages\/[^/]+\/edit$/.test(route);
  let id: string | undefined;
  try {
    id = route.includes("/")
      ? decodeURIComponent(
          storageEditor
            ? route.split("/")[1]
            : route.slice(route.indexOf("/") + 1),
        )
      : undefined;
    if (route === "storages/create/new") id = undefined;
  } catch {
    return <Alert severity="error">Invalid resource address.</Alert>;
  }
  const content =
    route === "storages" || route.startsWith("storages/") ? (
      storageEditor && value.role === "reader" ? (
        <Alert severity="error">Write access required.</Alert>
      ) : (
        <Resources
          key={`${route}:${value.role}`}
          kind="storage"
          id={id}
          writable={value.role !== "reader"}
          editor={storageEditor}
        />
      )
    ) : route === "clients" || route.startsWith("clients/") ? (
      <Resources
        key={`${route}:${value.role}`}
        kind="client"
        id={id}
        writable={value.role !== "reader"}
      />
    ) : route === "accounts" ||
      route.startsWith("accounts/") ||
      route.startsWith("access") ? (
      value.role === "admin" ? (
        <Accounts
          key={`${route}:${value.session_id}`}
          id={route.startsWith("accounts/") ? id : undefined}
          session={value}
        />
      ) : (
        <Alert severity="error">Admin access required.</Alert>
      )
    ) : route.startsWith("activity") ? (
      <Activity
        key={`${fullRoute}:${value.role}:${value.session_id}`}
        route={fullRoute}
        admin={value.role === "admin"}
      />
    ) : route.startsWith("settings") ? (
      <Profile
        key={value.session_id}
        session={value}
        security={route === "settings/security"}
      />
    ) : route === "usage" ? (
      <UsageHistory />
    ) : !route ? (
      <Overview />
    ) : (
      <Alert severity="info">
        Page not found. <Button href="#">Overview</Button>
      </Alert>
    );
  return (
    <Layout
      session={value}
      name={profile.data?.display_name ?? "Account"}
      route={route}
      loggingOut={busy}
      onLogout={() => {
        if (busy) return;
        setBusy(true);
        setError("");
        void request(`${identity}/session`, { method: "DELETE" })
          .then(
            () => clearSession(cache),
            (failure) => {
              if (failure instanceof ApiError && failure.status === 401)
                clearSession(cache);
              else setError(message(failure));
            },
          )
          .finally(() => setBusy(false));
      }}
    >
      {error && <Alert severity="error">{error}</Alert>}
      <Suspense fallback={<LinearProgress aria-label="Loading page" />}>
        {content}
      </Suspense>
    </Layout>
  );
}
