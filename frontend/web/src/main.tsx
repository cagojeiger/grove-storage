import React from "react";
import ReactDOM from "react-dom/client";
import {
  QueryCache,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { App } from "./app/App";
import { ApiError } from "./api/http";
import "./design/theme.css";

const client = new QueryClient({
  queryCache: new QueryCache({
    onError: (error) => {
      if (error instanceof ApiError && error.status === 401) {
        void client.cancelQueries({ queryKey: ["overview"] }).then(() => {
          client.removeQueries({ queryKey: ["overview"] });
          client.setQueryData(["session"], null);
        });
      }
    },
  }),
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
