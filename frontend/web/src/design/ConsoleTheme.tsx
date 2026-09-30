import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { CacheProvider } from "@emotion/react";
import createCache from "@emotion/cache";
import {
  CssBaseline,
  ThemeProvider,
  createTheme,
  useMediaQuery,
} from "@mui/material";
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-500.css";
import "@fontsource/inter/latin-600.css";
import "@fontsource/inter/latin-700.css";

type Mode = "system" | "light" | "dark";
const Preference = createContext<{ mode: Mode; setMode: (mode: Mode) => void }>(
  { mode: "system", setMode: () => {} },
);
export const useThemePreference = () => useContext(Preference);
const nonce = document.querySelector<HTMLMetaElement>(
  'meta[name="csp-nonce"]',
)?.content;
const cache = createCache({
  key: "grove",
  nonce: nonce === "__GROVE_CSP_NONCE__" ? undefined : nonce,
});

export function ConsoleTheme({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<Mode>(() => {
    try {
      const saved = localStorage.getItem("grove-theme");
      return saved === "light" || saved === "dark" ? saved : "system";
    } catch {
      return "system";
    }
  });
  const systemDark = useMediaQuery("(prefers-color-scheme: dark)");
  const resolved = mode === "system" ? (systemDark ? "dark" : "light") : mode;
  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
    try {
      localStorage.setItem("grove-theme", mode);
    } catch {
      /* Preferences may be disabled. */
    }
  }, [mode, resolved]);
  const theme = useMemo(
    () =>
      createTheme({
        palette: {
          mode: resolved,
          primary: { main: resolved === "dark" ? "#9cdb79" : "#176b3d" },
          background: {
            default: resolved === "dark" ? "#18191c" : "#fafafb",
            paper: resolved === "dark" ? "#202126" : "#ffffff",
          },
          text: {
            primary: resolved === "dark" ? "#f0f0f3" : "#23252b",
            secondary: resolved === "dark" ? "#b0b1ba" : "#686b75",
          },
          divider: resolved === "dark" ? "#393b43" : "#e2e3e8",
          error: { main: resolved === "dark" ? "#ff9aad" : "#b3233e" },
        },
        shape: { borderRadius: 6 },
        typography: {
          fontFamily:
            'Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
          fontSize: 14,
          allVariants: { letterSpacing: 0 },
          h1: { fontSize: "1.5rem", fontWeight: 600, lineHeight: 1.4 },
          h2: { fontSize: "1rem", fontWeight: 600, lineHeight: 1.5 },
          h3: { fontSize: "0.875rem", fontWeight: 600 },
          body1: { fontSize: "0.875rem", lineHeight: 1.6 },
          body2: { fontSize: "0.8125rem", lineHeight: 1.5 },
          button: {
            textTransform: "none",
            fontWeight: 500,
            fontSize: "0.8125rem",
          },
        },
        components: {
          MuiButtonBase: {
            defaultProps: { disableRipple: true },
            styleOverrides: {
              root: {
                "&.Mui-focusVisible": {
                  outline: "2px solid",
                  outlineColor: resolved === "dark" ? "#9cdb79" : "#176b3d",
                  outlineOffset: 2,
                },
              },
            },
          },
          MuiCssBaseline: {
            styleOverrides: (theme) => ({
              ":root": {
                "--bg": theme.palette.background.default,
                "--surface": theme.palette.background.paper,
                "--text": theme.palette.text.primary,
                "--muted": theme.palette.text.secondary,
                "--line": theme.palette.divider,
                "--accent": theme.palette.primary.main,
                "--focus": theme.palette.primary.main,
                "--danger": theme.palette.error.main,
                "--selected": resolved === "dark" ? "#273b2a" : "#eaf5eb",
                "--green": resolved === "dark" ? "#7ed5a9" : "#237954",
                "--connection-line":
                  resolved === "dark" ? "#718779" : "#82958b",
                colorScheme: resolved,
              },
              h1: { ...theme.typography.h1 },
              h2: { ...theme.typography.h2 },
              h3: { ...theme.typography.h3 },
              a: { color: theme.palette.primary.main },
              "button, input, select, textarea": { letterSpacing: 0 },
              "fieldset:disabled .MuiInputBase-root, fieldset:disabled .MuiButtonBase-root":
                { opacity: 0.6 },
            }),
          },
          MuiButton: {
            defaultProps: {
              variant: "outlined",
              disableElevation: true,
              size: "small",
            },
            styleOverrides: {
              root: {
                minHeight: 36,
                gap: 8,
                paddingInline: 14,
                whiteSpace: "normal",
                "@media (pointer: coarse)": { minHeight: 44 },
              },
            },
          },
          MuiIconButton: {
            defaultProps: { size: "small" },
            styleOverrides: {
              root: {
                width: 36,
                height: 36,
                borderRadius: 6,
                "@media (pointer: coarse)": { width: 44, height: 44 },
              },
            },
          },
          MuiOutlinedInput: {
            defaultProps: { size: "small" },
            styleOverrides: {
              root: {
                fontSize: "0.875rem",
                backgroundColor: resolved === "dark" ? "#202126" : "#fff",
              },
              input: { padding: "10px 12px", minWidth: 0 },
              multiline: { padding: "10px 12px" },
            },
          },
          MuiCheckbox: {
            defaultProps: { size: "small" },
            styleOverrides: { root: { padding: 6 } },
          },
          MuiDialog: {
            defaultProps: { fullWidth: true, maxWidth: "sm" },
            styleOverrides: {
              paper: {
                margin: 16,
                maxHeight: "calc(100dvh - 32px)",
                width: "calc(100% - 32px)",
                backgroundImage: "none",
              },
            },
          },
          MuiDialogTitle: {
            styleOverrides: {
              root: {
                fontSize: "1rem",
                fontWeight: 600,
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                gap: 16,
              },
            },
          },
          MuiDialogContent: { styleOverrides: { root: { paddingBottom: 24 } } },
          MuiTab: {
            styleOverrides: {
              root: {
                textTransform: "none",
                minHeight: 44,
                padding: "10px 16px",
                fontSize: "0.8125rem",
              },
            },
          },
          MuiTabs: {
            styleOverrides: {
              root: {
                minHeight: 44,
                borderBottom: "1px solid",
                borderColor: resolved === "dark" ? "#393b43" : "#e2e3e8",
                marginBlock: 24,
              },
            },
          },
          MuiListItemButton: {
            styleOverrides: {
              root: {
                borderRadius: 6,
                gap: 12,
                minHeight: 40,
                fontSize: "0.8125rem",
              },
            },
          },
        },
      }),
    [resolved],
  );
  return (
    <CacheProvider value={cache}>
      <Preference.Provider value={{ mode, setMode }}>
        <ThemeProvider theme={theme}>
          <CssBaseline />
          {children}
        </ThemeProvider>
      </Preference.Provider>
    </CacheProvider>
  );
}
