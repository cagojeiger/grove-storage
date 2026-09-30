import { useId, ReactNode } from "react";
import {
  Dialog as MuiDialog,
  DialogTitle,
  IconButton,
  Tooltip,
} from "@mui/material";
import { X } from "lucide-react";

export function Dialog({
  title,
  busy,
  onClose,
  children,
  closeDisabled = false,
}: {
  title: string;
  busy: boolean;
  onClose: () => void;
  children: ReactNode;
  closeDisabled?: boolean;
}) {
  const label = useId();
  return (
    <MuiDialog
      open
      aria-labelledby={label}
      slotProps={{
        paper: {
          sx: {
            "& > form": {
              display: "flex",
              flexDirection: "column",
              overflow: "hidden",
              minHeight: 0,
            },
          },
        },
      }}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !busy && !closeDisabled) onClose();
      }}
    >
      <DialogTitle
        id={`${label}-heading`}
        sx={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 2,
        }}
      >
        <span id={label}>{title}</span>
        <Tooltip title="Close">
          <span>
            <IconButton
              type="button"
              className="icon-button"
              aria-label="Close"
              title="Close"
              disabled={busy || closeDisabled}
              onClick={onClose}
            >
              <X size={18} />
            </IconButton>
          </span>
        </Tooltip>
      </DialogTitle>
      {children}
    </MuiDialog>
  );
}
