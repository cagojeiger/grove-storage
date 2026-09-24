import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut, LayoutDashboard } from "lucide-react";
import { admin, ApiError, message, request, Session } from "../api/http";
import { Login } from "../auth/Login";
import { ThemePicker } from "../design/Theme";
import { Overview } from "../features/overview/Overview";

export function App() {
  const cache = useQueryClient();
  const [loggingOut, setLoggingOut] = useState(false);
  const [logoutError, setLogoutError] = useState("");
  const session = useQuery({
    queryKey: ["session"],
    queryFn: async ({ signal }) => {
      try {
        return await request<Session>(`${admin}/session`, { signal });
      } catch (error) {
        if (error instanceof ApiError && error.status === 401) {
          cache.removeQueries({ queryKey: ["overview"] });
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
      await request(`${admin}/session`, { method: "DELETE" });
      await cache.cancelQueries();
      cache.clear();
      cache.setQueryData(["session"], null);
    } catch (error) {
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
            <div className="nav-current">
              <LayoutDashboard size={18} />
              <span>개요</span>
            </div>
            <span className="admin-label">관리자</span>
          </aside>
          <div className="content">
            {logoutError && (
              <p className="logout-error" role="alert">
                {logoutError}
              </p>
            )}
            <Overview />
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
