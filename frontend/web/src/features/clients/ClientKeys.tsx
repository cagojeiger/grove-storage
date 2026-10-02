import {
  Alert,
  Button,
  IconButton,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
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
    <Stack component="section" aria-label="S3 credentials" spacing={2}>
      <Stack
        direction={{ xs: "column", sm: "row" }}
        spacing={1}
        sx={{
          alignItems: { xs: "flex-start", sm: "center" },
          justifyContent: "space-between",
        }}
      >
        <Typography component="h2" variant="h6">
          S3 Credentials
        </Typography>
        <Button
          type="button"
          variant="contained"
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
        <TableContainer>
          <Table
            size="small"
            aria-label="Issued S3 credentials"
            sx={{ tableLayout: "fixed" }}
          >
            <TableHead>
              <TableRow>
                <TableCell>Access key ID</TableCell>
                <TableCell align="right" sx={{ width: 80 }}>
                  Actions
                </TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {query.data.map((key) => (
                <TableRow key={key} hover>
                  <TableCell>
                    <Typography
                      component="code"
                      variant="body2"
                      sx={{
                        fontFamily: "monospace",
                        overflowWrap: "anywhere",
                      }}
                    >
                      {key}
                    </Typography>
                  </TableCell>
                  <TableCell align="right">
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
                  </TableCell>
                </TableRow>
              ))}
              {!query.data.length && (
                <TableRow>
                  <TableCell colSpan={2}>No credentials issued.</TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </TableContainer>
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
