import { Button } from "@mui/material";
import { Input } from "../../design/Fields";
import { useState } from "react";
import { Copy } from "lucide-react";
import { Issued } from "../../api/identity";

export function IssuedToken({
  value,
  onDone,
}: {
  value: Issued;
  onDone: () => void;
}) {
  const [saved, setSaved] = useState(false);
  const [notice, setNotice] = useState("");
  return (
    <section className="issued-token" aria-label="Issued token">
      <h2>Token issued</h2>
      <label>
        Token
        <Input
          aria-label="Issued token"
          readOnly
          value={value.token}
          autoComplete="off"
          spellCheck={false}
        />
      </label>
      <Button
        type="button"
        className="action-button"
        onClick={() => {
          void navigator.clipboard.writeText(value.token).then(
            () => setNotice("Copied"),
            () =>
              setNotice("Clipboard unavailable. Select the token to copy it."),
          );
        }}
      >
        <Copy size={16} />
        Copy token
      </Button>
      <p role="status">{notice}</p>
      <dl>
        <dt>Account ID</dt>
        <dd>{value.account_id ?? value.user_id}</dd>
        <dt>Expires</dt>
        <dd>{new Date(value.expires_at).toLocaleString("en-US")}</dd>
      </dl>
      <div className="token-connections">
        <strong>CLI</strong>
        <code>gscli --endpoint {window.location.origin} --token-file &lt;token-file&gt; status</code>
        <strong>MCP</strong>
        <code>{window.location.origin}/api/admin/mcp</code>
        <code>Authorization: Bearer &lt;token&gt;</code>
      </div>
      <label className="check-field">
        <Input
          type="checkbox"
          checked={saved}
          onChange={(e) => setSaved(e.target.checked)}
        />
        I have saved this token. It is shown only once.
      </label>
      <Button type="button" onClick={onDone} disabled={!saved}>
        Done
      </Button>
    </section>
  );
}
