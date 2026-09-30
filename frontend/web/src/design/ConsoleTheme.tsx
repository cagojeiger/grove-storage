import type { ReactNode } from "react";
import { CacheProvider } from "@emotion/react";
import createCache from "@emotion/cache";
import { CssBaseline, ThemeProvider, createTheme } from "@mui/material";
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-500.css";
import "@fontsource/inter/latin-600.css";
import "@fontsource/inter/latin-700.css";

const nonce = document.querySelector<HTMLMetaElement>(
  'meta[name="csp-nonce"]',
)?.content;
const cache = createCache({
  key: "grove",
  nonce: nonce === "__GROVE_CSP_NONCE__" ? undefined : nonce,
});

const theme = createTheme({
  cssVariables: { colorSchemeSelector: '[data-theme="%s"]' },
  colorSchemes: {
    light: {
      palette: {
        primary: { main: "#176b3d" },
        background: { default: "#fafafb", paper: "#ffffff" },
        text: { primary: "#23252b", secondary: "#686b75" },
        divider: "#e2e3e8",
        error: { main: "#b3233e" },
      },
    },
    dark: {
      palette: {
        primary: { main: "#9cdb79" },
        background: { default: "#18191c", paper: "#202126" },
        text: { primary: "#f0f0f3", secondary: "#b0b1ba" },
        divider: "#393b43",
        error: { main: "#ff9aad" },
      },
    },
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
    button: { textTransform: "none", fontWeight: 500 },
  },
  components: {
    MuiCssBaseline: {
      styleOverrides: (theme) => ({
        // Keep the custom Overview and existing detail layouts on the same palette.
        ":root": {
          "--bg": theme.vars.palette.background.default,
          "--surface": theme.vars.palette.background.paper,
          "--text": theme.vars.palette.text.primary,
          "--muted": theme.vars.palette.text.secondary,
          "--line": theme.vars.palette.divider,
          "--accent": theme.vars.palette.primary.main,
          "--focus": theme.vars.palette.primary.main,
          "--danger": theme.vars.palette.error.main,
          "--selected": "#eaf5eb",
          "--green": "#237954",
          "--connection-line": "#82958b",
        },
        '[data-theme="dark"]': {
          "--selected": "#273b2a",
          "--green": "#7ed5a9",
          "--connection-line": "#718779",
        },
        h1: theme.typography.h1,
        h2: theme.typography.h2,
        h3: theme.typography.h3,
        a: { color: theme.vars.palette.primary.main },
        "button, input, select, textarea": { letterSpacing: 0 },
        "fieldset:disabled .MuiInputBase-root, fieldset:disabled .MuiButtonBase-root":
          { opacity: 0.6 },
      }),
    },
    MuiButton: {
      defaultProps: { disableElevation: true, size: "small" },
      styleOverrides: {
        root: {
          gap: 8,
          minHeight: 36,
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
          "@media (pointer: coarse)": { width: 44, height: 44 },
        },
      },
    },
    MuiTextField: {
      defaultProps: { fullWidth: true, size: "small", variant: "outlined" },
    },
    MuiFormControl: { defaultProps: { size: "small" } },
    MuiCheckbox: { defaultProps: { size: "small" } },
    MuiFormControlLabel: {
      styleOverrides: { root: { marginLeft: 0, marginRight: 0 } },
    },
    MuiDialog: {
      defaultProps: { fullWidth: true, maxWidth: "sm" },
      styleOverrides: {
        paper: {
          margin: 16,
          width: "calc(100% - 32px)",
          maxHeight: "calc(100dvh - 32px)",
        },
      },
    },
    MuiDialogTitle: {
      styleOverrides: { root: { fontSize: "1rem", fontWeight: 600 } },
    },
    MuiDialogActions: { styleOverrides: { root: { padding: "16px 24px" } } },
    MuiTableCell: { styleOverrides: { head: { fontWeight: 600 } } },
    MuiListItemButton: { styleOverrides: { root: { borderRadius: 6 } } },
  },
});

export function ConsoleTheme({ children }: { children: ReactNode }) {
  return (
    <CacheProvider value={cache}>
      <ThemeProvider
        theme={theme}
        defaultMode="system"
        modeStorageKey="grove-theme"
      >
        <CssBaseline enableColorScheme />
        {children}
      </ThemeProvider>
    </CacheProvider>
  );
}
