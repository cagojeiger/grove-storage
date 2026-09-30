import {
  DialogContent,
  DialogActions,
  Stack,
  Typography,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
} from "@mui/material";

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
    <>
      <DialogContent>
        <Stack component="section" aria-label="Issued token" spacing={3}>
          <Typography component="h2" variant="h6">
            Token issued
          </Typography>
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
            startIcon={<Copy size={16} />}
            onClick={() => {
              if (!navigator.clipboard) {
                setNotice(
                  "Clipboard unavailable. Select the token to copy it.",
                );
                return;
              }
              void navigator.clipboard.writeText(value.token).then(
                () => setNotice("Copied"),
                () =>
                  setNotice(
                    "Clipboard unavailable. Select the token to copy it.",
                  ),
              );
            }}
          >
            Copy token
          </Button>
          <Typography role="status" variant="body2">
            {notice}
          </Typography>
          <Stack component="dl" spacing={2}>
            {[
              ["Account ID", value.account_id ?? value.user_id],
              ["Expires", new Date(value.expires_at).toLocaleString("en-US")],
            ].map(([label, content]) => (
              <div key={label}>
                <Typography
                  component="dt"
                  variant="body2"
                  color="text.secondary"
                >
                  {label}
                </Typography>
                <Typography
                  component="dd"
                  sx={{ m: 0, overflowWrap: "anywhere" }}
                >
                  {content}
                </Typography>
              </div>
            ))}
          </Stack>
          <Stack spacing={1} sx={{ overflowWrap: "anywhere" }}>
            <Typography variant="subtitle2">CLI</Typography>
            <Typography
              component="code"
              variant="body2"
              sx={{ fontFamily: "monospace" }}
            >
              gscli --endpoint {window.location.origin} --token-file
              &lt;token-file&gt; status
            </Typography>
            <Typography variant="subtitle2">MCP</Typography>
            <Typography
              component="code"
              variant="body2"
              sx={{ fontFamily: "monospace" }}
            >
              {window.location.origin}/api/admin/mcp
            </Typography>
            <Typography
              component="code"
              variant="body2"
              sx={{ fontFamily: "monospace" }}
            >
              Authorization: Bearer &lt;token&gt;
            </Typography>
          </Stack>
        </Stack>
      </DialogContent>
      <DialogActions
        sx={{ flexDirection: "column", alignItems: "stretch" }}
        disableSpacing
      >
        <FormControlLabel
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
      </DialogActions>
    </>
  );
}
