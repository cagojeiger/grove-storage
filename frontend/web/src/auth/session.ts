import { QueryClient } from "@tanstack/react-query";

export function clearSession(cache: QueryClient) {
  void cache.cancelQueries();
  cache.removeQueries({
    predicate: (query) => query.queryKey[0] !== "session",
  });
  cache.setQueryData(["session"], null);
}
