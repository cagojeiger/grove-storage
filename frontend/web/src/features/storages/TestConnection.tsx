import { Alert, Button, Stack, Typography } from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import { PlugZap } from "lucide-react";
import { command } from "../../api/commands";
import { ApiError, message } from "../../api/http";

export function TestConnection({
  id,
  revision,
  refreshing,
}: {
  id: string;
  revision: number;
  refreshing: boolean;
}) {
  const check = useQuery({
    queryKey: ["storages", "connection", id, revision],
    queryFn: ({ signal }) =>
      command<{ id: string; state: "ok" }>("storage.test", { id }, signal),
    enabled: false,
    retry: false,
    gcTime: 0,
  });
  const error =
    check.error instanceof ApiError && check.error.status === 409
      ? "Storage settings changed during the test. Refresh and test again."
      : check.error instanceof ApiError && check.error.status === 503
        ? "Connection could not be verified. Check the endpoint, bucket, credentials and server connectivity."
        : message(check.error);
  return (
    <Stack
      component="section"
      aria-label="Storage connection"
      spacing={2}
      sx={{ pt: 3, borderTop: 1, borderColor: "divider" }}
    >
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={1}
        sx={{
          alignItems: { xs: "flex-start", sm: "center" },
          justifyContent: "space-between",
        }}
      >
        <Typography component="h2" variant="h6">Connection</Typography>
        <Button
          type="button"
          variant="outlined"
          startIcon={<PlugZap size={17} />}
          disabled={check.isFetching || refreshing}
          onClick={() => void check.refetch()}
        >
          {check.isFetching ? "Testing..." : "Test connection"}
        </Button>
      </Stack>
      {check.isFetching ? (
        <Typography role="status" variant="body2">
          Checking bucket access...
        </Typography>
      ) : check.isError ? (
        <Alert severity="error">{error}</Alert>
      ) : check.isSuccess ? (
        <Alert severity="success" role="status">
          Bucket access verified at{" "}
          {new Date(check.dataUpdatedAt).toLocaleTimeString("en-US")}. Upload,
          download and public URL not tested.
        </Alert>
      ) : (
        <Typography variant="body2" color="text.secondary">
          Not checked
        </Typography>
      )}
    </Stack>
  );
}
