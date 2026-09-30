import { IconButton, Button, Table, TableHead, TableBody, TableRow, TableCell } from "@mui/material";
import { Input } from "../../design/Fields";
import { FormEvent, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, RefreshCw } from "lucide-react";
import { command } from "../../api/commands";
import { message } from "../../api/http";
import { bytes } from "../../design/format";

type Snapshot = {
  day: string;
  storage_id: string;
  client_id: string;
  active_files: number;
  active_bytes: number;
};

export function UsageHistory() {
  const [days, setDays] = useState(90);
  const [visible, setVisible] = useState(100);
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
  function apply(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const value = Number(new FormData(event.currentTarget).get("days"));
    if (!Number.isInteger(value) || value < 1 || value > 3650) return;
    setVisible(100);
    if (value === days) void query.refetch();
    else setDays(value);
  }
  return (
    <main className="overview usage-history">
      <a className="back-link" href="#">
        <ArrowLeft size={16} />
        Overview
      </a>
      <div className="page-heading">
        <div>
          <p className="eyebrow">USAGE</p>
          <h1>Usage history</h1>
        </div>
        <IconButton type="submit"
          className="icon-button"
          title="Refresh usage history"
          aria-label="Refresh usage history"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          <RefreshCw size={18} />
        </IconButton>
      </div>
      <form className="usage-range" onSubmit={apply}>
        <label>
          Days
          <Input
            name="days"
            type="number"
            min="1"
            max="3650"
            step="1"
            required
            defaultValue={90}
          />
        </label>
        <Button type="submit" disabled={query.isFetching}>
          Apply
        </Button>
      </form>
      {query.isPending ? (
        <p role="status">Loading usage history...</p>
      ) : query.isError ? (
        <p role="alert">{message(query.error)}</p>
      ) : !rows.length ? (
        <p className="empty">No snapshots recorded for this period.</p>
      ) : (
        <>
          <div
            className="usage-table"
            role="region"
            aria-label="Daily snapshots"
            tabIndex={0}
          >
            <Table>
              <caption>Daily snapshots (UTC)</caption>
              <TableHead>
                <TableRow>
                  <TableCell scope="col">Date</TableCell>
                  <TableCell scope="col">Storage</TableCell>
                  <TableCell scope="col">Client</TableCell>
                  <TableCell scope="col">Active files</TableCell>
                  <TableCell scope="col">Active data</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {rows.slice(0, visible).map((row) => (
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
                    <TableCell>{row.active_files.toLocaleString("en-US")}</TableCell>
                    <TableCell>{bytes(row.active_bytes)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
          <div className="usage-count">
            <span className="muted">
              {Math.min(visible, rows.length).toLocaleString("en-US")} of{" "}
              {rows.length.toLocaleString("en-US")} snapshots
            </span>
            {visible < rows.length && (
              <Button type="submit" onClick={() => setVisible(visible + 100)}>
                Show more
              </Button>
            )}
          </div>
        </>
      )}
    </main>
  );
}
