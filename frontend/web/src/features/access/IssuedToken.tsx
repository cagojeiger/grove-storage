import { TextField, Checkbox, FormControlLabel, Button } from "@mui/material";

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
      <TextField
        value={value.token}
        autoComplete="off"
        label={"Token"}
        slotProps={{
          htmlInput: { "aria-label": "Issued token", spellCheck: false },
          input: { readOnly: true },
        }}
      />
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
        <code>
          gscli --endpoint {window.location.origin} --token-file
          &lt;token-file&gt; status
        </code>
        <strong>MCP</strong>
        <code>{window.location.origin}/api/admin/mcp</code>
        <code>Authorization: Bearer &lt;token&gt;</code>
      </div>
      <FormControlLabel
        className="check-field"
        control={
          <Checkbox
            checked={saved}
            onChange={(e) => setSaved(e.target.checked)}
          />
        }
        label={"I have saved this token. It is shown only once."}
      />
      <Button type="button" onClick={onDone} disabled={!saved}>
        Done
      </Button>
    </section>
  );
}
