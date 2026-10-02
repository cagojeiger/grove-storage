import {
  TextField,
  IconButton,
  Button,
  Table,
  TableHead,
  TableBody,
  TableRow,
  TableCell,
  Alert,
  Stack,
  TableContainer,
  TablePagination,
  Tooltip,
  Typography,
} from "@mui/material";

import { FormEvent, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { Page } from "../../app/Page";
import { command } from "../../api/commands";
import { message } from "../../api/http";
import { bytes } from "../../design/format";
import { Snapshot } from "./usageHistoryModel";
import { UsageChart } from "./UsageChart";

export function UsageHistory() {
  const [days, setDays] = useState(90);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const query = useQuery({
    queryKey: ["usage-history", days],
    queryFn: ({ signal }) =>
      command<Snapshot[]>("usage.history", { days }, signal),
    gcTime: 0,
  });
  const rows = [...(query.data ?? [])].sort(
    (a, b) =>
      b.day.localeCompare(a.day) ||
      a.storage_id.localeCompare(b.storage_id) ||
      a.client_id.localeCompare(b.client_id),
  );
  const currentPage = Math.min(
    page,
    Math.max(0, Math.ceil(rows.length / pageSize) - 1),
  );
  function apply(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const value = Number(new FormData(event.currentTarget).get("days"));
    if (!Number.isInteger(value) || value < 1 || value > 3650) return;
    setPage(0);
    if (value === days) void query.refetch();
    else setDays(value);
  }
  return (
    <Page
      title="Usage history"
      back={{ label: "Overview", href: "#" }}
      actions={
        <Tooltip title="Refresh usage history">
          <span>
            <IconButton
              aria-label="Refresh usage history"
              disabled={query.isFetching}
              onClick={() => void query.refetch()}
            >
              <RefreshCw size={18} />
            </IconButton>
          </span>
        </Tooltip>
      }
    >
      <Stack
        component="form"
        direction="row"
        spacing={2}
        onSubmit={apply}
        sx={{ alignItems: "center" }}
      >
        <TextField
          sx={{ maxWidth: 160 }}
          name="days"
          type="number"
          required
          defaultValue={90}
          label={"Days"}
          slotProps={{ htmlInput: { min: "1", max: "3650", step: "1" } }}
        />
        <Button type="submit" disabled={query.isFetching}>
          Apply
        </Button>
      </Stack>
      {query.isPending ? (
        <Typography role="status">Loading usage history...</Typography>
      ) : query.isError ? (
        <Alert severity="error">{message(query.error)}</Alert>
      ) : !rows.length ? (
        <Typography color="text.secondary">
          No snapshots recorded for this period.
        </Typography>
      ) : (
        <>
          <UsageChart rows={query.data ?? []} />
          <TableContainer
            role="region"
            aria-label="Daily snapshots"
            tabIndex={0}
          >
            <Table
              size="small"
              sx={{
                minWidth: 620,
                overflowWrap: "anywhere",
                tableLayout: "fixed",
              }}
            >
              <caption>Daily snapshots (UTC)</caption>
              <TableHead>
                <TableRow>
                  <TableCell scope="col">Date</TableCell>
                  <TableCell scope="col">Storage</TableCell>
                  <TableCell scope="col">Client</TableCell>
                  <TableCell scope="col">Stored files</TableCell>
                  <TableCell scope="col">Stored data</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {rows
                  .slice(currentPage * pageSize, (currentPage + 1) * pageSize)
                  .map((row) => (
                    <TableRow
                      key={JSON.stringify([
                        row.day,
                        row.storage_id,
                        row.client_id,
                      ])}
                    >
                      <TableCell>
                        <time dateTime={row.day}>{row.day}</time>
                      </TableCell>
                      <TableCell>{row.storage_id}</TableCell>
                      <TableCell>{row.client_id}</TableCell>
                      <TableCell>
                        {row.active_files.toLocaleString("en-US")}
                      </TableCell>
                      <TableCell>{bytes(row.active_bytes)}</TableCell>
                    </TableRow>
                  ))}
              </TableBody>
            </Table>
          </TableContainer>
          <TablePagination
            component="div"
            slotProps={{ toolbar: { sx: { flexWrap: "wrap" } } }}
            count={rows.length}
            page={currentPage}
            rowsPerPage={pageSize}
            rowsPerPageOptions={[20, 50, 100]}
            onPageChange={(_event, next) => setPage(next)}
            onRowsPerPageChange={(event) => {
              setPageSize(Number(event.target.value));
              setPage(0);
            }}
          />
        </>
      )}
    </Page>
  );
}
