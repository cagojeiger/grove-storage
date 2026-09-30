import { Monitor, Moon, Sun } from "lucide-react";
import { Select } from "./Fields";
import { useThemePreference } from "./ConsoleTheme";

type Theme = "system" | "light" | "dark";
export function ThemePicker() {
  const { mode: theme, setMode: setTheme } = useThemePreference();
  const Icon = theme === "dark" ? Moon : theme === "light" ? Sun : Monitor;
  return (
    <label className="theme">
      <Icon size={16} aria-hidden="true" />
      <Select
        aria-label="Theme"
        value={theme}
        onChange={(event) => setTheme(event.target.value as Theme)}
      >
        <option value="system">System</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
      </Select>
    </label>
  );
}
