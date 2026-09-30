import { Button, ListItemButton, Popover, Divider } from "@mui/material";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronUp, CircleUserRound, LogOut, LayoutDashboard, HardDrive, Shield, AppWindow, ScrollText, Settings } from "lucide-react";
import { identity, ApiError, currentSession, message, request } from "../api/http";
import { identityRequest, isAccount } from "../api/identity";
import { Login } from "../auth/Login";
import { SetPassword } from "../auth/SetPassword";
import { ThemePicker } from "../design/Theme";
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
  const accessPage = route === "accounts" || route.startsWith("accounts/") || route === "access" || route.startsWith("access/");
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
        </div>
      </header>
      {route === "set-password" ? (
        <SetPassword key={fullRoute} />
      ) : session.isPending ? (
        <main className="connection" role="status">
          Checking session...
        </main>
      ) : session.isError ? (
        <main className="connection">
          <p role="alert">{message(session.error)}</p>
          <Button type="submit" onClick={() => void session.refetch()}>Reconnect</Button>
        </main>
      ) : session.data ? (
        <div className="workspace">
          <aside aria-label="Workspace sidebar">
            <nav aria-label="Main navigation">
              <ListItemButton component="a"
                href="#"
                aria-current={!storagePage && !accessPage && !clientPage && !activityPage && !settingsPage ? "page" : undefined}
              >
                <LayoutDashboard size={18} />
                <span>Overview</span>
              </ListItemButton>
              <p className="nav-group">Resources</p>
              <ListItemButton component="a"
                href="#storages"
                aria-current={storagePage ? "page" : undefined}
              >
                <HardDrive size={18} />
                <span>Storage</span>
              </ListItemButton>
              <ListItemButton component="a" href="#clients" aria-current={clientPage ? "page" : undefined}><AppWindow size={18} /><span>Clients</span></ListItemButton>
              <p className="nav-group">Management</p>
              {session.data.role === "admin" && (
                <ListItemButton component="a"
                  href="#accounts"
                  aria-current={accessPage ? "page" : undefined}
                >
                  <Shield size={18} />
                  <span>Accounts</span>
                </ListItemButton>
              )}
              <ListItemButton component="a" href="#activity" aria-current={activityPage ? "page" : undefined}><ScrollText size={18} /><span>Activity</span></ListItemButton>
            </nav>
            <div className="sidebar-account">
              <span className="admin-label">{{ reader: "Reader · Read-only", writer: "Writer · Operations", admin: "Admin · Management" }[session.data.role]}</span>
              <AccountMenu
                name={profile.data?.display_name ?? "Account"}
                role={session.data.role}
                passwordSession={session.data.credential_id === null}
                loggingOut={loggingOut}
                onLogout={() => void logout()}
              />
            </div>
          </aside>
          <div className="content">
            {logoutError && (
              <p className="logout-error" role="alert">
                {logoutError}
              </p>
            )}
            {accessPage ? (
              session.data.role === "admin" ? (
                <Access key={`${route}:${session.data.session_id}`} route={route} currentUserId={session.data.user_id} />
              ) : (
                <main className="connection"><p role="alert">Admin access required.</p></main>
              )
            ) : activityPage ? (
              <Activity key={`${fullRoute}:${session.data.role}:${session.data.session_id}`} route={fullRoute} admin={session.data.role === "admin"} />
            ) : settingsPage ? (
              route === "settings/security"
                ? session.data.credential_id === null
                  ? <Security />
                  : <main className="connection"><p role="alert">Password sign-in required.</p></main>
                : <Sessions key={session.data.session_id} session={session.data} />
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
      <footer className="console-footer">
        <span>Grove Storage</span>
      </footer>
    </>
  );
}

function AccountMenu({ name, role, passwordSession, loggingOut, onLogout }: {
  name: string;
  role: string;
  passwordSession: boolean;
  loggingOut: boolean;
  onLogout: () => void;
}) {
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const close = () => setAnchor(null);
  return <div className="account-menu">
    <Button variant="text" className="account-menu-trigger" aria-label="Account menu" aria-haspopup="dialog" aria-expanded={Boolean(anchor)} onClick={(event) => setAnchor(event.currentTarget)}><CircleUserRound size={18} /><span>{name}</span><ChevronUp size={16} /></Button>
    <Popover open={Boolean(anchor)} anchorEl={anchor} onClose={close} anchorOrigin={{ vertical: "top", horizontal: "left" }} transformOrigin={{ vertical: "bottom", horizontal: "left" }} slotProps={{ paper: { className: "account-menu-panel", role: "dialog", "aria-label": "Account menu" } }}>
      <div className="account-menu-identity"><strong>{name}</strong><span>{role}</span></div>
      <Divider />
      <ListItemButton component="a" href="#settings" onClick={close}><CircleUserRound size={16} />My account</ListItemButton>
      {passwordSession && <ListItemButton component="a" href="#settings/security" onClick={close}><Settings size={16} />Security</ListItemButton>}
      <ListItemButton component="button" onClick={() => { close(); onLogout(); }} disabled={loggingOut}><LogOut size={16} />Sign out</ListItemButton>
    </Popover>
  </div>;
}
