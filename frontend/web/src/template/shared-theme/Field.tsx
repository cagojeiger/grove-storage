import { useId } from "react";
import {
  FormControl,
  FormLabel,
  TextField,
  type TextFieldProps,
} from "@mui/material";

// The official Sign-in form uses labels above inputs, not floating labels.
export default function Field({
  label,
  id,
  fullWidth,
  sx,
  ...props
}: TextFieldProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  return (
    <FormControl
      fullWidth={fullWidth}
      sx={sx}
      error={props.error}
      disabled={props.disabled}
      required={props.required}
    >
      {label && <FormLabel htmlFor={inputId}>{label}</FormLabel>}
      <TextField {...props} id={inputId} fullWidth={fullWidth} />
    </FormControl>
  );
}
