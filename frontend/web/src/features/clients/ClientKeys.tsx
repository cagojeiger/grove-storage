import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";
import { command } from "../../api/commands";
import { message } from "../../api/http";
import { KeyAction, KeyDialog } from "./KeyDialog";

export function ClientKeys({ clientId }: { clientId: string }) {
  const [action, setAction] = useState<KeyAction | null>(null);
  const native = useQuery({
    queryKey: ["clients", "keys", clientId, "native"],
    queryFn: ({ signal }) =>
      command<string[]>("client-key.list", { client_id: clientId }, signal),
  });
  const s3 = useQuery({
    queryKey: ["clients", "keys", clientId, "s3"],
    queryFn: ({ signal }) =>
      command<string[]>("credential.list", { client_id: clientId }, signal),
  });
  return (
    <>
      {(["native", "s3"] as const).map((kind) => {
        const query = kind === "native" ? native : s3;
        return (
          <section
            className="service-keys"
            aria-label={kind === "native" ? "Native keys" : "S3 keys"}
            key={kind}
          >
            <div className="section-heading">
              <h2>{kind === "native" ? "Native keys" : "S3 keys"}</h2>
              <div className="page-actions">
                {kind === "native" && (
                  <button
                    onClick={() => setAction({ kind: "native-register" })}
                  >
                    <Plus size={16} />
                    Register key
                  </button>
                )}
                <button
                  onClick={() =>
                    setAction({
                      kind: kind === "native" ? "native-generate" : "s3-create",
                    })
                  }
                >
                  <Plus size={16} />
                  {kind === "native" ? "Generate key" : "Issue key"}
                </button>
              </div>
            </div>
            {query.isPending ? (
              <p role="status">Loading keys...</p>
            ) : query.isError ? (
              <p role="alert">
                {message(query.error)}{" "}
                <button onClick={() => void query.refetch()}>Retry keys</button>
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
                      onClick={() =>
                        setAction({
                          kind:
                            kind === "native" ? "native-delete" : "s3-delete",
                          key,
                        })
                      }
                    >
                      <Trash2 size={16} />
                    </button>
                  </div>
                ))}
                {!query.data.length && (
                  <p className="empty">No keys registered.</p>
                )}
              </>
            )}
          </section>
        );
      })}
      {action && (
        <KeyDialog
          clientId={clientId}
          action={action}
          onClose={() => setAction(null)}
        />
      )}
    </>
  );
}
