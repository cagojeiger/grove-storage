import { useQueries } from "@tanstack/react-query";
import {
  Link,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
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
    <TableContainer>
      <Table size="small" aria-label="Clients" sx={{ tableLayout: "fixed" }}>
        <TableHead>
          <TableRow>
            <TableCell>Client</TableCell>
            <TableCell sx={{ display: { xs: "none", md: "table-cell" } }}>
              Storage
            </TableCell>
            <TableCell
              align="right"
              sx={{ display: { xs: "none", sm: "table-cell" } }}
            >
              Active files
            </TableCell>
            <TableCell align="right">Active data</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {ids.map((id, i) => {
            const summary = usage ? totals(usage, id) : undefined;
            const storage = details[i].isError
              ? "Unavailable"
              : (details[i].data?.storage_id ?? "Loading...");
            const files = summary
              ? `${summary.files.toLocaleString("en-US")} files`
              : "Unavailable";
            return (
              <TableRow hover key={id}>
                <TableCell sx={{ overflowWrap: "anywhere" }}>
                  <Link href={href(clientLink(id))}>{id}</Link>
                  <Typography
                    variant="body2"
                    color="text.secondary"
                    sx={{ display: { xs: "block", md: "none" } }}
                  >
                    {storage}
                  </Typography>
                </TableCell>
                <TableCell
                  sx={{
                    display: { xs: "none", md: "table-cell" },
                    overflowWrap: "anywhere",
                  }}
                >
                  {storage}
                </TableCell>
                <TableCell
                  align="right"
                  sx={{
                    display: { xs: "none", sm: "table-cell" },
                    fontVariantNumeric: "tabular-nums",
                  }}
                >
                  {files}
                </TableCell>
                <TableCell
                  align="right"
                  sx={{ fontVariantNumeric: "tabular-nums" }}
                >
                  {summary ? bytes(summary.bytes) : "Unavailable"}
                  <Typography
                    variant="body2"
                    color="text.secondary"
                    sx={{ display: { xs: "block", sm: "none" } }}
                  >
                    {files}
                  </Typography>
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </TableContainer>
  );
}
