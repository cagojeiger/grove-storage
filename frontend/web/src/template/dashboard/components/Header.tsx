import Stack from "@mui/material/Stack";
import NavbarBreadcrumbs from "./NavbarBreadcrumbs";
import ColorModeIconDropdown from "../../shared-theme/ColorModeIconDropdown";
export default function Header({ route }: { route: string }) {
  return (
    <Stack
      component="header"
      direction="row"
      spacing={2}
      sx={{
        display: { xs: "none", md: "flex" },
        width: "100%",
        alignItems: "center",
        justifyContent: "space-between",
        maxWidth: 1700,
        pt: 1.5,
      }}
    >
      <NavbarBreadcrumbs route={route} />
      <ColorModeIconDropdown />
    </Stack>
  );
}
