import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut, LayoutDashboard, HardDrive, Shield } from "lucide-react";
import { identity, ApiError, currentSession, message, request } from "../api/http";
import { Login } from "../auth/Login";
import { ThemePicker } from "../design/Theme";
import { Overview } from "../features/overview/Overview";
import { Storages } from "../features/storages/Storages";
import { clearSession } from "../auth/session";
import { useRoute } from "./navigation";
import { Access } from "../features/access/Access";
import { MasterSetup } from "../auth/MasterSetup";

export function App() {
  const cache = useQueryClient();
  const route = useRoute();
  const storagePage = route === "storages" || route.startsWith("storages/");
  const accessPage = route === "access" || route.startsWith("access/");
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
      ) : session.data ? (
        <div className="workspace">
          <aside>
            <nav aria-label="Main navigation">
              <a
                href="#"
                aria-current={!storagePage && !accessPage ? "page" : undefined}
              >
                <LayoutDashboard size={18} />
                <span>Overview</span>
              </a>
              <a
                href="#storages"
                aria-current={storagePage ? "page" : undefined}
              >
                <HardDrive size={18} />
                <span>Storage</span>
              </a>
              {session.data.role === "admin" && (
                <a
                  href="#access/users"
                  aria-current={accessPage ? "page" : undefined}
                >
                  <Shield size={18} />
                  <span>Access</span>
                </a>
              )}
            </nav>
            <span className="admin-label">{{ viewer: "Viewer · Read-only", operator: "Operator · Operations", admin: "Admin · Management" }[session.data.role]}</span>
          </aside>
          <div className="content">
            {logoutError && (
              <p className="logout-error" role="alert">
                {logoutError}
              </p>
            )}
            {accessPage ? (
              session.data.role === "admin" ? (
                <Access />
              ) : (
                <main className="connection"><p role="alert">Admin access required.</p></main>
              )
            ) : storagePage ? (
              <Storages key={`${route}:${session.data.role}`} route={route} canWrite={session.data.role !== "viewer"} />
            ) : (
              <Overview />
            )}
          </div>
        </div>
      ) : route === "setup" ? (
        <MasterSetup />
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
