import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../../api/http";
import { identityMessage } from "../../api/identity";
import { clearSession } from "../../auth/session";

export function useAction() {
  const cache = useQueryClient();
  const lock = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [unknown, setUnknown] = useState(false);
  async function run(work: () => Promise<void>) {
    if (lock.current || unknown) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      await work();
    } catch (e) {
      setError(identityMessage(e));
      setUnknown(!(e instanceof ApiError) || e.outcome !== "not_applied");
      if (e instanceof ApiError && e.status === 401) clearSession(cache);
      if (e instanceof ApiError && e.status === 403) {
        cache.removeQueries({ queryKey: ["access"] });
        void cache.invalidateQueries({ queryKey: ["session"] });
      }
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  return { busy, error, unknown, run };
}
