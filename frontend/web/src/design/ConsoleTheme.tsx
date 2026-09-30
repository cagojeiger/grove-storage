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
      },
    },
    dark: {
      palette: {
        primary: { main: "#9cdb79" },
      },
    },
  },
  typography: {
    fontFamily:
      'Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    allVariants: { letterSpacing: 0 },
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
        // Legacy semantic headings use the same stock scale as Typography.
        h1: theme.typography.h5,
        h2: theme.typography.h6,
        h3: theme.typography.subtitle1,
        dt: {
          ...theme.typography.body2,
          color: theme.vars.palette.text.secondary,
        },
        ".detail-fields": {
          gap: theme.spacing(2, 3),
          paddingTop: theme.spacing(2),
        },
        ".detail-fields dt": {
          ...theme.typography.body2,
          color: theme.vars.palette.text.secondary,
        },
        ".detail-fields dd": {
          ...theme.typography.body1,
          marginTop: theme.spacing(0.5),
          overflowWrap: "anywhere",
        },
        a: { color: theme.vars.palette.primary.main },
        "button, input, select, textarea": { letterSpacing: 0 },
      }),
    },
    MuiTextField: {
      defaultProps: { fullWidth: true },
    },
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
