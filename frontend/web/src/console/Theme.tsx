import { type ReactNode } from "react";
import { CacheProvider } from "@emotion/react";
import createCache from "@emotion/cache";
import { CssBaseline } from "@mui/material";
import { cspNonce } from "../app/csp";
import AppTheme from "../template/shared-theme/AppTheme";
export { default as ThemeMode } from "../template/shared-theme/ColorModeIconDropdown";

const cache = createCache({ key: "grove", nonce: cspNonce });
export function Theme({ children }: { children: ReactNode }) {
  return (
    <CacheProvider value={cache}>
      <AppTheme>
        <CssBaseline enableColorScheme />
        {children}
      </AppTheme>
    </CacheProvider>
  );
}
