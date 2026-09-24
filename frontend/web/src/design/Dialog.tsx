import { useEffect, useId, useRef, ReactNode } from "react";
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
  const ref = useRef<HTMLDialogElement>(null);
  const label = useId();
  useEffect(() => {
    const dialog = ref.current!;
    const trigger = document.activeElement;
    dialog.showModal();
    return () => {
      dialog.close();
      if (trigger instanceof HTMLElement && trigger.isConnected)
        trigger.focus();
    };
  }, []);
  return (
    <dialog
      ref={ref}
      aria-labelledby={label}
      onCancel={(event) => {
        event.preventDefault();
        if (!busy && !closeDisabled) onClose();
      }}
    >
      <div className="dialog-heading">
        <h2 id={label}>{title}</h2>
        <button
          type="button"
          className="icon-button"
          aria-label="Close"
          title="Close"
          disabled={busy || closeDisabled}
          onClick={onClose}
        >
          <X size={18} />
        </button>
      </div>
      {children}
    </dialog>
  );
}
