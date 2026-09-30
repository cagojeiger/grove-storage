import { useQueries } from "@tanstack/react-query";
import { Link, Table, TableBody, TableCell, TableContainer, TableHead, TableRow } from "@mui/material";
import { command } from "../../api/commands";
import { bytes } from "../../design/format";
import { Client, ClientUsage, clientLink, totals } from "./model";

export function ClientRows({
  ids,
  usage,
  href,
}: {
  ids: string[];
  usage?: ClientUsage[];
  href: (link: string) => string;
}) {
  const details = useQueries({
    queries: ids.map((id) => ({
      queryKey: ["clients", "detail", id],
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        command<Client>("client.show", { id }, signal),
    })),
  });
  return (
    <TableContainer><Table size="small" aria-label="Clients" sx={{ minWidth: 520 }}>
      <TableHead><TableRow><TableCell>Client</TableCell><TableCell>Storage</TableCell><TableCell align="right">Active files</TableCell><TableCell align="right">Active data</TableCell></TableRow></TableHead>
      <TableBody>
      {ids.map((id, i) => {
        const summary = usage ? totals(usage, id) : undefined;
        return (
          <TableRow hover key={id}>
            <TableCell><Link href={href(clientLink(id))}>{id}</Link></TableCell>
            <TableCell>
              {details[i].isError
                ? "Unavailable"
                : (details[i].data?.storage_id ?? "Loading...")}
            </TableCell>
            <TableCell align="right">
              {summary
                ? `${summary.files.toLocaleString("en-US")} files`
                : "Unavailable"}
            </TableCell>
            <TableCell align="right">
              {summary ? bytes(summary.bytes) : "Unavailable"}
            </TableCell>
          </TableRow>
        );
      })}
      </TableBody>
    </Table></TableContainer>
  );
}
