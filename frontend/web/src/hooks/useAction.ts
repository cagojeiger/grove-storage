import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../api/http";
import { identityMessage } from "../api/identity";
import { clearSession } from "../auth/session";

export function useAction(
  errorMessage = identityMessage,
  reauthenticate = false,
) {
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
      setError(errorMessage(e));
      const rejected =
        e instanceof ApiError &&
        (e.outcome === "not_applied" ||
          (reauthenticate &&
            [400, 401, 403, 404, 409, 429].includes(e.status)));
      setUnknown(!rejected);
      if (e instanceof ApiError && e.status === 401 && !reauthenticate)
        clearSession(cache);
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
