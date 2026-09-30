import React from "react";
import ReactDOM from "react-dom/client";
import {
  QueryCache,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { App } from "./app/App";
import { ConsoleTheme } from "./design/ConsoleTheme";
import { ApiError } from "./api/http";
import { clearSession } from "./auth/session";
import "./design/theme.css";

const client = new QueryClient({
  queryCache: new QueryCache({
    onError: (error) => {
      if (error instanceof ApiError && error.status === 401) {
        clearSession(client);
      }
    },
  }),
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={client}>
      <ConsoleTheme><App /></ConsoleTheme>
    </QueryClientProvider>
  </React.StrictMode>,
);
