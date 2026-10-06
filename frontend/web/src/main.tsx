import React from "react";
import ReactDOM from "react-dom/client";
import {
  QueryCache,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { App } from "./console/App";
import { Theme } from "./console/Theme";
import { ErrorBoundary } from "./console/ErrorBoundary";
import { ApiError } from "./api/http";
import { clearSession } from "./auth/session";

const client = new QueryClient({
  queryCache: new QueryCache({
    onError: (error, query) => {
      if (error instanceof ApiError && error.status === 401) {
        clearSession(client);
      }
      if (
        error instanceof ApiError &&
        error.status === 403 &&
        query.queryKey[0] !== "session"
      ) {
        void client.invalidateQueries({ queryKey: ["session"] });
      }
    },
  }),
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={client}>
      <Theme>
        <ErrorBoundary>
          <App />
        </ErrorBoundary>
      </Theme>
    </QueryClientProvider>
  </React.StrictMode>,
);
