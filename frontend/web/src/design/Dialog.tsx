import { useId, ReactNode } from "react";
import {
  Dialog as MuiDialog,
  DialogContent,
  DialogTitle,
  IconButton,
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
    </MuiDialog>
  );
}
