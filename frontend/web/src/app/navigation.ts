import { useSyncExternalStore } from "react";

function subscribe(listener: () => void) {
  window.addEventListener("hashchange", listener);
  return () => window.removeEventListener("hashchange", listener);
}

export function useRoute() {
  return useSyncExternalStore(subscribe, () => window.location.hash.slice(1));
}

export function storageLink(id: string) {
  return `#storages/${encodeURIComponent(id)}`;
}
