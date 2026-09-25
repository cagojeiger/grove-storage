import { useQuery } from "@tanstack/react-query";
import { ChevronRight, LockKeyhole } from "lucide-react";
import { identityRequest, isObject } from "../../api/identity";
import { message } from "../../api/http";

type Root = { id: "root"; configured: boolean; protected: true; source: "config" };
export function RootAccount({ selected, onSelect }: { selected: boolean; onSelect: () => void }) {
  const query = useQuery({
    queryKey: ["access", "root"],
    queryFn: ({ signal }) => identityRequest("/root", (v): v is Root =>
      isObject(v) && v.id === "root" && v.source === "config" && v.protected === true && typeof v.configured === "boolean", { signal }),
  });
  if (query.isPending) return <p role="status">Loading Root account...</p>;
  if (query.isError) return <p role="alert">{message(query.error)} <button onClick={() => void query.refetch()}>Retry Root account</button></p>;
  const status = query.data.configured ? "Configured" : "Not configured";
  if (!selected) return <button className="account-row" onClick={onSelect}>
    <span><strong>Root</strong><span className="muted">Config</span></span><span>Protected</span><span>{status}</span><ChevronRight size={16} />
  </button>;
  return <section aria-label="Root account">
    <h2><LockKeyhole size={20} /> Root</h2>
    <dl className="detail-fields">
      <div><dt>Role</dt><dd>Root</dd></div>
      <div><dt>Source</dt><dd>Server configuration</dd></div>
      <div><dt>Status</dt><dd>{status}</dd></div>
      <div><dt>Protection</dt><dd>Managed by server configuration</dd></div>
    </dl>
    <a className="back-link" href="#setup">Setup &amp; recovery</a>
  </section>;
}
