import { useQueries } from "@tanstack/react-query";
import { ButtonBase } from "@mui/material";
import { ChevronRight } from "lucide-react";
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
    <div className="client-registry">
      <div className="client-columns" aria-hidden="true">
        <span>Client</span>
        <span>Storage</span>
        <span>Active files</span>
        <span>Active data</span>
        <span />
      </div>
      {ids.map((id, i) => {
        const summary = usage ? totals(usage, id) : undefined;
        return (
          <ButtonBase component="a" className="client-row" key={id} href={href(clientLink(id))}>
            <strong>{id}</strong>
            <span>
              <span className="mobile-label">Storage</span>
              {details[i].isError
                ? "Unavailable"
                : (details[i].data?.storage_id ?? "Loading...")}
            </span>
            <span>
              <span className="mobile-label">Active files</span>
              {summary
                ? `${summary.files.toLocaleString("en-US")} files`
                : "Unavailable"}
            </span>
            <span>
              <span className="mobile-label">Active data</span>
              {summary ? bytes(summary.bytes) : "Unavailable"}
            </span>
            <ChevronRight size={16} />
          </ButtonBase>
        );
      })}
    </div>
  );
}
