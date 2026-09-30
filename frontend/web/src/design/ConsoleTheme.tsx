import type { ReactNode } from "react";
import { CacheProvider } from "@emotion/react";
import createCache from "@emotion/cache";
import { CssBaseline, ThemeProvider, createTheme } from "@mui/material";
import { cspNonce } from "../app/csp";

const cache = createCache({
  key: "grove",
  nonce: cspNonce,
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
    fontFamily: [
      "-apple-system",
      "BlinkMacSystemFont",
      '"Segoe UI"',
      "Roboto",
      '"Helvetica Neue"',
      "Arial",
      "sans-serif",
    ].join(","),
    allVariants: { letterSpacing: 0 },
  },
  components: {
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
