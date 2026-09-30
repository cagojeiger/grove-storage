import { Button } from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import { PlugZap } from "lucide-react";
import { command } from "../../api/commands";
import { ApiError, message } from "../../api/http";

export function TestConnection({ id, revision, refreshing }: { id: string; revision: number; refreshing: boolean }) {
  const check = useQuery({
    queryKey: ["storages", "connection", id, revision],
    queryFn: ({ signal }) => command<{ id: string; state: "ok" }>("storage.test", { id }, signal),
    enabled: false,
    retry: false,
    gcTime: 0,
  });
  const error = check.error instanceof ApiError && check.error.status === 409
    ? "Storage settings changed during the test. Refresh and test again."
    : check.error instanceof ApiError && check.error.status === 503
      ? "Connection could not be verified. Check the endpoint, bucket, credentials and server connectivity."
      : message(check.error);
  return (
    <section className="storage-section" aria-label="Storage connection">
      <div className="section-heading">
        <h2>Connection</h2>
        <Button type="submit" disabled={check.isFetching || refreshing} onClick={() => void check.refetch()}>
          <PlugZap size={17} />
          {check.isFetching ? "Testing..." : "Test connection"}
        </Button>
      </div>
      {check.isFetching ? <p role="status">Checking bucket access...</p>
        : check.isError ? <p role="alert">{error}</p>
        : check.isSuccess ? <p role="status">Bucket access verified at {new Date(check.dataUpdatedAt).toLocaleTimeString("en-US")}. Upload, download and public URL not tested.</p>
        : <p className="muted">Not checked</p>}
    </section>
  );
}
