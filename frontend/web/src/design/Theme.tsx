import { Monitor, Moon, Sun } from "lucide-react";
import { NativeSelect, Stack, useColorScheme } from "@mui/material";

type Theme = "system" | "light" | "dark";
export function ThemePicker() {
  const { mode: theme = "system", setMode: setTheme } = useColorScheme();
  const Icon = theme === "dark" ? Moon : theme === "light" ? Sun : Monitor;
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
      <Icon size={16} aria-hidden="true" />
      <NativeSelect
        inputProps={{ "aria-label": "Theme" }}
        disableUnderline
        value={theme}
        onChange={(event) => setTheme(event.target.value as Theme)}
      >
        <option value="system">System</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </NativeSelect>
    </Stack>
  );
}
