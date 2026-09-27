import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut, LayoutDashboard, HardDrive, Shield, AppWindow, ScrollText, Settings } from "lucide-react";
import { identity, ApiError, currentSession, message, request } from "../api/http";
import { Login } from "../auth/Login";
import { ThemePicker } from "../design/Theme";
import { Overview } from "../features/overview/Overview";
import { Storages } from "../features/storages/Storages";
import { clearSession } from "../auth/session";
import { useRoute } from "./navigation";
import { Access } from "../features/access/Access";
import { MasterSetup } from "../auth/MasterSetup";
import { Clients } from "../features/clients/Clients";
import { Activity } from "../features/activity/Activity";
import { Sessions } from "../features/settings/Sessions";
import { UsageHistory } from "../features/overview/UsageHistory";

export function App() {
  const cache = useQueryClient();
  const fullRoute = useRoute();
  const route = fullRoute.split("?")[0];
  const storagePage = route === "storages" || route.startsWith("storages/");
  const accessPage = route === "accounts" || route.startsWith("accounts/") || route === "access" || route.startsWith("access/");
  const clientPage = route === "clients" || route.startsWith("clients/");
  const activityPage = route === "activity" || route.startsWith("activity/");
  const settingsPage = route === "settings";
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
    <>
      <header>
        <a className="brand" href={import.meta.env.BASE_URL}>
          <img
            src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
            alt=""
          />
          <span>Grove Storage</span>
        </a>
        <div className="header-actions">
          <ThemePicker />
          {session.data && (
            <button
              className="icon-button"
              title="Sign out"
              aria-label="Sign out"
              onClick={() => void logout()}
              disabled={loggingOut}
            >
              <LogOut size={18} />
            </button>
          )}
        </div>
      </header>
      {session.isPending ? (
        <main className="connection" role="status">
          Checking session...
        </main>
      ) : session.isError ? (
        <main className="connection">
          <p role="alert">{message(session.error)}</p>
          <button onClick={() => void session.refetch()}>Reconnect</button>
        </main>
      ) : route === "setup" ? (
        <MasterSetup />
      ) : session.data ? (
        <div className="workspace">
          <aside>
            <nav aria-label="Main navigation">
              <a
                href="#"
                aria-current={!storagePage && !accessPage && !clientPage && !activityPage && !settingsPage ? "page" : undefined}
              >
                <LayoutDashboard size={18} />
                <span>Overview</span>
              </a>
              <p className="nav-group">Resources</p>
              <a
                href="#storages"
                aria-current={storagePage ? "page" : undefined}
              >
                <HardDrive size={18} />
                <span>Storage</span>
              </a>
              <a href="#clients" aria-current={clientPage ? "page" : undefined}><AppWindow size={18} /><span>Clients</span></a>
              <p className="nav-group">Management</p>
              {["admin", "root"].includes(session.data.role) && (
                <a
                  href="#accounts"
                  aria-current={accessPage ? "page" : undefined}
                >
                  <Shield size={18} />
                  <span>Accounts</span>
                </a>
              )}
              <a href="#activity" aria-current={activityPage ? "page" : undefined}><ScrollText size={18} /><span>Activity</span></a>
              <a href="#settings" aria-current={settingsPage ? "page" : undefined}><Settings size={18} /><span>My account</span></a>
            </nav>
            <span className="admin-label">{{ reader: "Reader · Read-only", writer: "Writer · Operations", admin: "Admin · Management", root: "Root · Protected" }[session.data.role]}</span>
          </aside>
          <div className="content">
            {logoutError && (
              <p className="logout-error" role="alert">
                {logoutError}
              </p>
            )}
            {accessPage ? (
              ["admin", "root"].includes(session.data.role) ? (
                <Access key={`${route}:${session.data.session_id}`} route={route} currentUserId={session.data.principal === "user" ? session.data.user_id : undefined} />
              ) : (
                <main className="connection"><p role="alert">Admin access required.</p></main>
              )
            ) : activityPage ? (
              <Activity key={`${fullRoute}:${session.data.role}:${session.data.session_id}`} route={fullRoute} admin={["admin", "root"].includes(session.data.role)} />
            ) : settingsPage ? (
              <Sessions key={session.data.session_id} session={session.data} />
            ) : clientPage ? (
              <Clients key={`${route}:${session.data.role}`} route={route} canWrite={session.data.role !== "reader"} />
            ) : storagePage ? (
              <Storages key={`${route}:${session.data.role}`} route={route} canWrite={session.data.role !== "reader"} />
            ) : route === "usage" ? (
              <UsageHistory />
            ) : (
              <Overview />
            )}
          </div>
        </div>
      ) : (
        <Login
          onLogin={(value) => {
            setLogoutError("");
            cache.setQueryData(["session"], value);
          }}
        />
      )}
    </>
  );
}
