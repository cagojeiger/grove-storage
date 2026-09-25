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
        <button
          className="icon-button"
          title="Refresh usage history"
          aria-label="Refresh usage history"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          <RefreshCw size={18} />
        </button>
      </div>
      <form className="usage-range" onSubmit={apply}>
        <label>
          Days
          <input
            name="days"
            type="number"
            min="1"
            max="3650"
            step="1"
            required
            defaultValue={90}
          />
        </label>
        <button type="submit" disabled={query.isFetching}>
          Apply
        </button>
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
            <table>
              <caption>Daily snapshots (UTC)</caption>
              <thead>
                <tr>
                  <th scope="col">Date</th>
                  <th scope="col">Storage</th>
                  <th scope="col">Client</th>
                  <th scope="col">Active files</th>
                  <th scope="col">Active data</th>
                </tr>
              </thead>
              <tbody>
                {rows.slice(0, visible).map((row) => (
                  <tr
                    key={JSON.stringify([
                      row.day,
                      row.storage_id,
                      row.client_id,
                    ])}
                  >
                    <td>
                      <time dateTime={row.day}>{row.day}</time>
                    </td>
                    <td>{row.storage_id}</td>
                    <td>{row.client_id}</td>
                    <td>{row.active_files.toLocaleString("en-US")}</td>
                    <td>{bytes(row.active_bytes)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="usage-count">
            <span className="muted">
              {Math.min(visible, rows.length).toLocaleString("en-US")} of{" "}
              {rows.length.toLocaleString("en-US")} snapshots
            </span>
            {visible < rows.length && (
              <button onClick={() => setVisible(visible + 100)}>
                Show more
              </button>
            )}
          </div>
        </>
      )}
    </main>
  );
}
