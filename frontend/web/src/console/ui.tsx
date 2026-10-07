import { type ReactNode, useId, useState } from "react";
import TextField from "../template/shared-theme/Field";
import {
  Alert,
  Box,
  Breadcrumbs,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControlLabel,
  IconButton,
  InputAdornment,
  LinearProgress,
  Link,
  Stack,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";
import { Copy, Eye, EyeOff, RefreshCw } from "lucide-react";
import { message } from "../api/http";
import { bytes } from "../design/format";
import { useAction } from "../hooks/useAction";

export function Page({
  title,
  parent,
  actions,
  children,
  fill = false,
}: {
  title: string;
  parent?: { label: string; href: string };
  actions?: ReactNode;
  children: ReactNode;
  fill?: boolean;
}) {
  return (
    <Stack
      spacing={2}
      sx={{ width: "100%", ...(fill && { flex: 1, minHeight: 0 }) }}
    >
      <Stack
        direction="row"
        spacing={2}
        useFlexGap
        sx={{
          alignItems: "center",
          justifyContent: "space-between",
          flexWrap: "wrap",
        }}
      >
        {parent ? (
          <Breadcrumbs aria-label="Page path" sx={{ minWidth: 0 }}>
            <Link href={parent.href}>{parent.label}</Link>
            <Typography
              variant="h5"
              component="h1"
              sx={{ overflowWrap: "anywhere" }}
            >
              {title}
            </Typography>
          </Breadcrumbs>
        ) : (
          <Typography
            variant="h5"
            component="h1"
            sx={{ overflowWrap: "anywhere" }}
          >
            {title}
          </Typography>
        )}
        <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap" }}>
          {actions}
        </Stack>
      </Stack>
      {children}
    </Stack>
  );
}
export function Refresh({
  onClick,
  disabled,
  label = "Refresh",
}: {
  onClick: () => void;
  disabled?: boolean;
  label?: string;
}) {
  return (
    <Tooltip title={label}>
      <span>
        <IconButton aria-label={label} disabled={disabled} onClick={onClick}>
          <RefreshCw size={20} />
        </IconButton>
      </span>
    </Tooltip>
  );
}
export function Properties({ values }: { values: Record<string, ReactNode> }) {
  return (
    <Box
      component="dl"
      sx={{
        m: 0,
        display: "grid",
        gridTemplateColumns: { xs: "1fr", sm: "minmax(140px, 1fr) 3fr" },
        columnGap: 3,
        rowGap: 1.5,
      }}
    >
      {Object.entries(values).map(([label, value]) => (
        <Box key={label} sx={{ display: "contents" }}>
          <Typography component="dt" variant="body2" color="text.secondary">
            {label}
          </Typography>
          <Typography
            component="dd"
            variant="body2"
            sx={{ m: 0, overflowWrap: "anywhere" }}
          >
            {value ?? "—"}
          </Typography>
        </Box>
      ))}
    </Box>
  );
}
export function QueryState({
  pending,
  error,
  retry,
}: {
  pending: boolean;
  error: unknown;
  retry?: () => void;
}) {
  if (pending) return <LinearProgress aria-label="Loading" />;
  if (error)
    return (
      <Alert
        severity="error"
        action={retry && <Button onClick={retry}>Retry</Button>}
      >
        {message(error)}
      </Alert>
    );
  return null;
}
export function Capacity({
  used,
  capacity,
}: {
  used: number;
  capacity: number;
}) {
  const percent = capacity > 0 ? (used / capacity) * 100 : null;
  return (
    <Stack spacing={1} sx={{ minWidth: 100 }}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="body2">{bytes(used)}</Typography>
        <Typography variant="body2" color="text.secondary">
          / {bytes(capacity)}
        </Typography>
      </Stack>
      {percent !== null && (
        <LinearProgress
          aria-label="Configured capacity usage"
          aria-valuetext={`${Math.round(percent)}% of configured capacity`}
          variant="determinate"
          value={Math.max(0, Math.min(100, percent))}
          color={used > capacity ? "error" : "primary"}
        />
      )}
      <Typography
        variant="caption"
        color={used > capacity ? "error" : "text.secondary"}
      >
        {used > capacity
          ? `Over capacity by ${bytes(used - capacity)}`
          : `${bytes(capacity - used)} remaining`}
      </Typography>
    </Stack>
  );
}
export function FormDialog({
  title,
  children,
  onClose,
  submit,
  action,
  button = "Save",
  disabled,
  danger = false,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  submit: (data: FormData, form: HTMLFormElement) => Promise<void>;
  action: ReturnType<typeof useAction>;
  button?: string;
  disabled?: boolean;
  danger?: boolean;
}) {
  const id = useId();
  const mobile = useMediaQuery(useTheme().breakpoints.down("sm"));
  return (
    <Dialog
      open
      fullWidth
      maxWidth="sm"
      fullScreen={mobile}
      aria-labelledby={id}
      onClose={(_, reason) => {
        if (!action.busy && reason === "escapeKeyDown") onClose();
      }}
    >
      <DialogTitle id={id}>{title}</DialogTitle>
      <DialogContent dividers>
        <Stack
          component="form"
          id={`${id}-form`}
          spacing={2.5}
          onSubmit={(event) => {
            event.preventDefault();
            const form = event.currentTarget;
            const data = new FormData(form);
            void action.run(() => submit(data, form));
          }}
        >
          <Box
            component="fieldset"
            disabled={action.busy || action.unknown}
            sx={{ border: 0, m: 0, p: 0, minWidth: 0 }}
          >
            <Stack spacing={2.5}>{children}</Stack>
          </Box>
          {action.error && <Alert severity="error">{action.error}</Alert>}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button disabled={action.busy} onClick={onClose}>
          {action.unknown ? "Close and review" : "Cancel"}
        </Button>
        <Button
          variant="contained"
          color={danger ? "error" : "primary"}
          type="submit"
          form={`${id}-form`}
          disabled={action.busy || action.unknown || disabled}
        >
          {action.busy ? "Saving..." : button}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
export function Confirmation({
  title,
  text,
  confirm,
  children,
  onClose,
  execute,
  errorMessage,
  label = "Confirmation",
  button = "Confirm",
}: {
  title: string;
  text: string;
  confirm: string;
  children?: ReactNode;
  onClose: () => void;
  execute: () => Promise<void>;
  errorMessage?: (error: unknown) => string;
  label?: string;
  button?: string;
}) {
  const action = useAction(errorMessage);
  const [value, setValue] = useState("");
  return (
    <FormDialog
      title={title}
      action={action}
      onClose={onClose}
      danger
      button={button}
      disabled={value !== confirm}
      submit={async () => {
        if (value === confirm) {
          setValue("");
          await execute();
        }
      }}
    >
      <Typography variant="body2">{text}</Typography>
      <TextField
        fullWidth
        label={label}
        helperText={`Enter ${confirm}`}
        value={value}
        onChange={(event) => setValue(event.target.value)}
        autoComplete="off"
        required
      />
      {children}
    </FormDialog>
  );
}
export function SecretDialog({
  title,
  values,
  details,
  onClose,
  acknowledgement = "I have saved this token. It is shown only once.",
}: {
  title: string;
  values: Record<string, string>;
  details?: ReactNode;
  onClose: () => void;
  acknowledgement?: string;
}) {
  const id = useId();
  const [saved, setSaved] = useState(false);
  const [notice, setNotice] = useState("");
  const [revealed, setRevealed] = useState<string[]>([]);
  return (
    <Dialog open fullWidth maxWidth="sm" aria-labelledby={id}>
      <DialogTitle id={id}>{title}</DialogTitle>
      <DialogContent dividers>
        <Stack spacing={3}>
          <Alert severity="warning">
            Shown only once. Store it securely before closing.
          </Alert>
          {Object.entries(values).map(([label, value]) => (
            <TextField
              key={label}
              label={label}
              type={
                label === "Access key ID" || revealed.includes(label)
                  ? "text"
                  : "password"
              }
              autoComplete="off"
              fullWidth
              value={value}
              slotProps={{
                input: {
                  readOnly: true,
                  endAdornment: (
                    <InputAdornment position="end">
                      {label !== "Access key ID" && (
                        <Tooltip title={`${revealed.includes(label) ? "Hide" : "Show"} ${label.toLowerCase()}`}>
                          <IconButton
                            aria-label={`${revealed.includes(label) ? "Hide" : "Show"} ${label.toLowerCase()}`}
                            onClick={() =>
                              setRevealed((current) =>
                                current.includes(label)
                                  ? current.filter((item) => item !== label)
                                  : [...current, label],
                              )
                            }
                          >
                            {revealed.includes(label) ? (
                              <EyeOff size={18} />
                            ) : (
                              <Eye size={18} />
                            )}
                          </IconButton>
                        </Tooltip>
                      )}
                      <Tooltip title={`Copy ${label.toLowerCase()}`}>
                        <IconButton
                          aria-label={`Copy ${label.toLowerCase()}`}
                          onClick={() => {
                            if (!navigator.clipboard) {
                              setNotice(
                                "Copy unavailable. Select the value manually.",
                              );
                              return;
                            }
                            void navigator.clipboard.writeText(value).then(
                              () => setNotice("Copied."),
                              () =>
                                setNotice(
                                  "Copy failed. Select the value manually.",
                                ),
                            );
                          }}
                        >
                          <Copy size={18} />
                        </IconButton>
                      </Tooltip>
                    </InputAdornment>
                  ),
                },
                htmlInput: {
                  onFocus: (event: React.FocusEvent<HTMLInputElement>) =>
                    event.currentTarget.select(),
                },
              }}
            />
          ))}
          {notice && (
            <Typography role="status" variant="body2">
              {notice}
            </Typography>
          )}
          {details && (
            <>
              <Divider />
              {details}
            </>
          )}
          <FormControlLabel
            control={
              <Checkbox
                checked={saved}
                onChange={(event) => setSaved(event.target.checked)}
              />
            }
            label={acknowledgement}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button variant="contained" disabled={!saved} onClick={onClose}>
          Done
        </Button>
      </DialogActions>
    </Dialog>
  );
}
