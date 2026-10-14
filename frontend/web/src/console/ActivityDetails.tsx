import { useId, useState } from "react";
import {
  Chip,
  DialogContent,
  DialogTitle,
  Divider,
  Drawer,
  IconButton,
  InputAdornment,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { Copy, X } from "lucide-react";
import TextField from "../template/shared-theme/Field";
import {
  actor,
  eventName,
  invocationStatus,
  sourceName,
  type Event,
} from "../features/activity/model";
import { duration, time } from "../design/format";
import { Properties } from "./ui";

export function ActivityDetails({ event, onClose }: {
  event: Event;
  onClose: () => void;
}) {
  const titleId = useId();
  const [notice, setNotice] = useState("");
  const outcome = "outcome" in event ? invocationStatus(event.outcome) : null;
  // Management history request_id is operation_id in operational logs.
  const operationId = event.context.request_id;
  return (
    <Drawer
      anchor="right"
      open
      onClose={onClose}
      slotProps={{ paper: {
        role: "dialog",
        "aria-labelledby": titleId,
        sx: { width: { xs: "100%", sm: 480 } },
      } }}
    >
      <Stack
        direction="row"
        sx={{ alignItems: "center", justifyContent: "space-between", pr: 2 }}
      >
        <DialogTitle id={titleId}>Event details</DialogTitle>
        <Tooltip title="Close">
          <IconButton aria-label="Close" onClick={onClose}>
            <X size={20} />
          </IconButton>
        </Tooltip>
      </Stack>
      <DialogContent dividers>
        <Stack spacing={3}>
          <Typography variant="subtitle1" sx={{ overflowWrap: "anywhere" }}>
            {eventName(event)}
          </Typography>
          <Properties values={{
            ...(outcome ? { Outcome: (
              <Chip
                label={outcome.label}
                color={outcome.color}
                size="small"
                variant="outlined"
              />
            ) } : {}),
            ...("resource_id" in event ? {
              Resource: event.resource_id,
              "Resource type": event.resource_type,
            } : {}),
            ...("reason_code" in event ? { Reason: event.reason_code } : {}),
            Time: time(event.context.created_at),
            Source: sourceName(event.context.surface),
            ...("duration_ms" in event ? {
              Duration: duration(event.duration_ms),
              "Error code": event.error_code,
            } : {}),
          }} />
          <Divider />
          <TextField
            fullWidth
            label="Operation ID"
            value={operationId}
            slotProps={{ input: {
            readOnly: true,
            endAdornment: <InputAdornment position="end">
              <Tooltip title="Copy operation ID">
                <IconButton
                  aria-label="Copy operation ID"
                  onClick={() => {
                    if (!navigator.clipboard) {
                      setNotice("Copy unavailable. Select the value manually.");
                      return;
                    }
                    void navigator.clipboard.writeText(operationId).then(
                      () => setNotice("Operation ID copied."),
                      () => setNotice("Copy failed. Select the value manually."),
                    );
                  }}
                >
                  <Copy size={18} />
                </IconButton>
              </Tooltip>
            </InputAdornment>,
            } }}
          />
          {notice && (
            <Typography role="status" variant="body2">{notice}</Typography>
          )}
          <Properties values={{
            Actor: actor(event.context),
            "Token ID": event.context.credential_id,
            "Session ID": event.context.session_id,
            "Event ID": event.context.id,
          }} />
          {"metadata" in event && Object.keys(event.metadata).length > 0 && (
            <Stack spacing={1}>
              <Typography variant="subtitle2">Metadata</Typography>
              <Typography
                component="pre"
                variant="body2"
                sx={{ m: 0, whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}
              >
                {JSON.stringify(event.metadata, null, 2)}
              </Typography>
            </Stack>
          )}
        </Stack>
      </DialogContent>
    </Drawer>
  );
}
