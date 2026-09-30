import {
  Alert,
  Box,
  Button,
  IconButton,
  List,
  ListItem,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";
import { command } from "../../api/commands";
import { message } from "../../api/http";
import { KeyAction, KeyDialog } from "./KeyDialog";

export function ClientKeys({ clientId }: { clientId: string }) {
  const [action, setAction] = useState<KeyAction | null>(null);
  const query = useQuery({
    queryKey: ["clients", "keys", clientId, "s3"],
    queryFn: ({ signal }) =>
      command<string[]>("credential.list", { client_id: clientId }, signal),
  });
  return (
    <Stack
      component="section"
      aria-label="S3 credentials"
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
        <Typography variant="h2">S3 Credentials</Typography>
        <Button
          type="button"
          variant="outlined"
          startIcon={<Plus size={16} />}
          onClick={() => setAction({ kind: "s3-create" })}
        >
          Create credential
        </Button>
      </Stack>
      {query.isPending ? (
        <Typography role="status" color="text.secondary">
          Loading credentials...
        </Typography>
      ) : query.isError ? (
        <Alert severity="error">
          {message(query.error)}{" "}
          <Button type="button" onClick={() => void query.refetch()}>
            Retry
          </Button>
        </Alert>
      ) : (
        <Box>
          <List disablePadding aria-label="Issued S3 credentials">
            {query.data.map((key) => (
              <ListItem disableGutters key={key} sx={{ gap: 2, py: 1 }}>
                <Typography
                  component="code"
                  variant="body2"
                  sx={{
                    fontFamily: "monospace",
                    overflowWrap: "anywhere",
                    minWidth: 0,
                    flex: 1,
                  }}
                >
                  {key}
                </Typography>
                <Tooltip title={`Revoke ${key}`}>
                  <IconButton
                    type="button"
                    color="error"
                    aria-label={`Revoke ${key}`}
                    onClick={() => setAction({ kind: "s3-delete", key })}
                  >
                    <Trash2 size={16} />
                  </IconButton>
                </Tooltip>
              </ListItem>
            ))}
          </List>
          {!query.data.length && (
            <Typography color="text.secondary" sx={{ py: 3 }}>
              No credentials issued.
            </Typography>
          )}
        </Box>
      )}
      {action && (
        <KeyDialog
          clientId={clientId}
          action={action}
          onClose={() => setAction(null)}
        />
      )}
    </Stack>
  );
}
