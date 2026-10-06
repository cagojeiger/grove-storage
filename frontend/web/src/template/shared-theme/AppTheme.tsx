import { type ReactNode } from "react";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import { inputsCustomizations } from "./customizations/inputs";
import { dataDisplayCustomizations } from "./customizations/dataDisplay";
import { feedbackCustomizations } from "./customizations/feedback";
import { navigationCustomizations } from "./customizations/navigation";
import { surfacesCustomizations } from "./customizations/surfaces";
import { colorSchemes, typography, shadows, shape } from "./themePrimitives";
import { dataGridCustomizations } from "../dashboard/theme/customizations/dataGrid";
import { chartsCustomizations } from "../dashboard/theme/customizations/charts";
const theme = createTheme({
  cssVariables: {
    colorSchemeSelector: '[data-theme="%s"]',
    cssVarPrefix: "template",
  },
  colorSchemes,
  typography,
  shadows,
  shape,
  components: {
    ...inputsCustomizations,
    ...dataDisplayCustomizations,
    ...feedbackCustomizations,
    ...navigationCustomizations,
    ...surfacesCustomizations,
    ...dataGridCustomizations,
    ...chartsCustomizations,
  },
});
export default function AppTheme({ children }: { children: ReactNode }) {
  return (
    <ThemeProvider
      theme={theme}
      defaultMode="system"
      modeStorageKey="grove-theme"
    >
      {children}
    </ThemeProvider>
  );
}
