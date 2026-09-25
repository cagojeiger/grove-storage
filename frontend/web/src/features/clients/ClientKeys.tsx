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
    <section className="service-keys" aria-label="S3 credentials">
      <div className="section-heading">
        <h2>S3 Credentials</h2>
        <button onClick={() => setAction({ kind: "s3-create" })}>
          <Plus size={16} />
          Create credential
        </button>
      </div>
      {query.isPending ? (
        <p role="status">Loading credentials...</p>
      ) : query.isError ? (
        <p role="alert">
          {message(query.error)}{" "}
          <button onClick={() => void query.refetch()}>Retry</button>
        </p>
      ) : (
        <>
          {query.data.map((key) => (
            <div className="service-key-row" key={key}>
              <code>{key}</code>
              <button
                className="icon-button danger"
                title={`Revoke ${key}`}
                aria-label={`Revoke ${key}`}
                onClick={() => setAction({ kind: "s3-delete", key })}
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
          {!query.data.length && (
            <p className="empty">No credentials issued.</p>
          )}
        </>
      )}
      {action && (
        <KeyDialog
          clientId={clientId}
          action={action}
          onClose={() => setAction(null)}
        />
      )}
    </section>
  );
}
