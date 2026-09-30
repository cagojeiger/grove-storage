import type {
  InputHTMLAttributes,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";
import { Checkbox, NativeSelect, OutlinedInput } from "@mui/material";

// Keep native constraints and FormData names on the actual control, not the MUI wrapper.
export function Input({
  className,
  value,
  defaultValue,
  onChange,
  disabled,
  required,
  readOnly,
  id,
  name,
  type,
  autoFocus,
  ...input
}: InputHTMLAttributes<HTMLInputElement>) {
  if (type === "checkbox")
    return (
      <Checkbox
        className={className}
        value={value}
        onChange={onChange}
        disabled={disabled}
        required={required}
        readOnly={readOnly}
        id={id}
        name={name}
        autoFocus={autoFocus}
        checked={input.checked}
        defaultChecked={input.defaultChecked}
        slotProps={{ input }}
      />
    );
  return (
    <OutlinedInput
      fullWidth
      className={className}
      value={value}
      defaultValue={defaultValue}
      onChange={onChange}
      disabled={disabled}
      required={required}
      readOnly={readOnly}
      id={id}
      name={name}
      type={type}
      autoFocus={autoFocus}
      slotProps={{ input }}
    />
  );
}

export function Select({
  children,
  className,
  value,
  defaultValue,
  onChange,
  disabled,
  required,
  id,
  name,
  ...input
}: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <NativeSelect
      className={className}
      value={value}
      defaultValue={defaultValue}
      onChange={onChange}
      disabled={disabled}
      required={required}
      id={id}
      name={name}
      input={<OutlinedInput />}
      inputProps={input}
    >
      {children}
    </NativeSelect>
  );
}

export function Textarea({
  className,
  value,
  defaultValue,
  onChange,
  disabled,
  required,
  readOnly,
  id,
  name,
  rows,
  ...input
}: Pick<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  | "className"
  | "value"
  | "defaultValue"
  | "onChange"
  | "disabled"
  | "required"
  | "readOnly"
  | "id"
  | "name"
  | "rows"
  | "spellCheck"
  | "autoComplete"
>) {
  return (
    <OutlinedInput
      fullWidth
      multiline
      rows={rows ?? 8}
      className={className}
      value={value}
      defaultValue={defaultValue}
      onChange={onChange}
      disabled={disabled}
      required={required}
      readOnly={readOnly}
      id={id}
      name={name}
      slotProps={{ input }}
    />
  );
}
