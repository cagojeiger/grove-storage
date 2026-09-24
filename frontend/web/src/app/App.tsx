import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut, LayoutDashboard, HardDrive } from "lucide-react";
import { identity, ApiError, currentSession, message, request } from "../api/http";
import { Login } from "../auth/Login";
import { ThemePicker } from "../design/Theme";
import { Overview } from "../features/overview/Overview";
import { Storages } from "../features/storages/Storages";
import { clearSession } from "../auth/session";
import { useRoute } from "./navigation";

export function App() {
  const cache = useQueryClient();
  const route = useRoute();
  const storagePage = route === "storages" || route.startsWith("storages/");
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
              title="로그아웃"
              aria-label="로그아웃"
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
          세션 확인 중...
        </main>
      ) : session.isError ? (
        <main className="connection">
          <p role="alert">{message(session.error)}</p>
          <button onClick={() => void session.refetch()}>다시 연결</button>
        </main>
      ) : session.data ? (
        <div className="workspace">
          <aside>
            <nav aria-label="주 메뉴">
              <a href="#" aria-current={!storagePage ? "page" : undefined}>
                <LayoutDashboard size={18} />
                <span>개요</span>
              </a>
              <a
                href="#storages"
                aria-current={storagePage ? "page" : undefined}
              >
                <HardDrive size={18} />
                <span>저장소</span>
              </a>
            </nav>
            <span className="admin-label">{{ viewer: "Viewer · 읽기", operator: "Operator · 운영", admin: "Admin · 관리" }[session.data.role]}</span>
          </aside>
          <div className="content">
            {logoutError && (
              <p className="logout-error" role="alert">
                {logoutError}
              </p>
            )}
            {storagePage ? (
              <Storages key={`${route}:${session.data.role}`} route={route} canWrite={session.data.role !== "viewer"} />
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
