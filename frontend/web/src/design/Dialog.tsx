import { useId, ReactNode } from "react";
import {
  Dialog as MuiDialog,
  DialogContent,
  DialogActions,
  DialogTitle,
  IconButton,
} from "@mui/material";
import { X } from "lucide-react";

export function Dialog({
  title,
  busy,
  onClose,
  children,
  actions,
  closeDisabled = false,
}: {
  title: string;
  busy: boolean;
  onClose: () => void;
  children: ReactNode;
  actions?: ReactNode;
  closeDisabled?: boolean;
}) {
  const label = useId();
  return (
    <MuiDialog
      open
      aria-labelledby={label}
      onClose={(_event, reason) => {
        if (reason === "escapeKeyDown" && !busy && !closeDisabled) onClose();
      }}
    >
      <DialogTitle>
        <span id={label}>{title}</span>
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
      </DialogTitle>
      <DialogContent>{children}</DialogContent>
      {actions && <DialogActions sx={{ px: 3, py: 2, borderTop: 1, borderColor: "divider", flexShrink: 0 }}>{actions}</DialogActions>}
    </MuiDialog>
  );
}
