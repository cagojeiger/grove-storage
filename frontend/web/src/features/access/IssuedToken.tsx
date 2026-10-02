import {
  DialogContent,
  DialogActions,
  Stack,
  Typography,
  TextField,
  Checkbox,
  FormControlLabel,
  Button,
  Alert,
  Accordion,
  AccordionSummary,
  AccordionDetails,
  InputAdornment,
  IconButton,
  Tooltip,
} from "@mui/material";

import { useState } from "react";
import { Copy, Eye, EyeOff, ChevronDown } from "lucide-react";
import { Issued } from "../../api/identity";

export function IssuedToken({
  value,
  onDone,
  accountName,
}: {
  value: Issued;
  onDone: () => void;
  accountName?: string;
}) {
  const [saved, setSaved] = useState(false);
  const [notice, setNotice] = useState("");
  const [revealed, setRevealed] = useState(false);
  function copy() {
    if (!navigator.clipboard) {
      setNotice("Clipboard unavailable. Reveal and select the token to copy it.");
      return;
    }
    void navigator.clipboard.writeText(value.token).then(
      () => setNotice("Copied"),
      () =>
        setNotice("Clipboard unavailable. Reveal and select the token to copy it."),
    );
  }
  return (
    <>
      <DialogContent dividers>
        <Stack component="section" aria-label="Token details" spacing={3}>
          <Alert severity="warning">This token is shown only once.</Alert>
          <TextField
            value={value.token}
            autoComplete="off"
            label={"Token"}
            type={revealed ? "text" : "password"}
            slotProps={{
              htmlInput: { "aria-label": "Issued token", spellCheck: false },
              input: {
                readOnly: true,
                sx: { fontFamily: "monospace" },
                endAdornment: (
                  <InputAdornment position="end">
                    <Tooltip title={revealed ? "Hide token" : "Show token"}>
                      <IconButton
                        aria-label={revealed ? "Hide token" : "Show token"}
                        onClick={() => setRevealed(!revealed)}
                      >
                        {revealed ? <EyeOff size={18} /> : <Eye size={18} />}
                      </IconButton>
                    </Tooltip>
                    <Tooltip title={notice === "Copied" ? "Copied" : "Copy token"}>
                      <IconButton aria-label="Copy token" onClick={copy}>
                        <Copy size={18} />
                      </IconButton>
                    </Tooltip>
                  </InputAdornment>
                ),
              },
            }}
          />
          {notice && (
            <Typography role="status" variant="body2">{notice}</Typography>
          )}
          <Stack component="dl" spacing={2}>
            {[
              [
                accountName ? "Account" : "Account ID",
                accountName ?? value.account_id ?? value.user_id,
              ],
              [
                "Expires",
                new Date(value.expires_at).toLocaleString("en-US", {
                  dateStyle: "medium",
                  timeStyle: "long",
                }),
              ],
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
          <Accordion>
            <AccordionSummary expandIcon={<ChevronDown size={18} />}>
              Connection examples
            </AccordionSummary>
            <AccordionDetails>
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
            </AccordionDetails>
          </Accordion>
          <FormControlLabel
            control={
              <Checkbox
                checked={saved}
                onChange={(e) => setSaved(e.target.checked)}
              />
            }
            label={"I have saved this token. It is shown only once."}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button variant="contained" type="button" onClick={onDone} disabled={!saved}>
          Done
        </Button>
      </DialogActions>
    </>
  );
}
