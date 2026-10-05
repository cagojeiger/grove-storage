import { useState } from "react";
import { AppBar, Box, Stack, Toolbar } from "@mui/material";
import { Menu } from "lucide-react";
import SideMenuMobile from "./SideMenuMobile";
import MenuButton from "./MenuButton";
import ColorModeIconDropdown from "../../shared-theme/ColorModeIconDropdown";
import { type NavigationProps } from "../Dashboard";
export default function AppNavbar(props: NavigationProps) {
  const [open, setOpen] = useState(false);
  return (
    <AppBar
      component="header"
      position="fixed"
      sx={{
        display: { xs: "block", md: "none" },
        boxShadow: 0,
        bgcolor: "background.paper",
        backgroundImage: "none",
        borderBottom: 1,
        borderColor: "divider",
      }}
    >
      <Toolbar sx={{ width: "100%", p: 1.5, gap: 1 }}>
        <Box
          component="img"
          src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
          alt="Grove Storage"
          sx={{ width: 32, height: 32 }}
        />
        <Stack direction="row" spacing={1} sx={{ ml: "auto" }}>
          <ColorModeIconDropdown />
          <MenuButton
            aria-label="Open navigation"
            onClick={() => setOpen(true)}
          >
            <Menu size={20} />
          </MenuButton>
        </Stack>
        <SideMenuMobile {...props} open={open} onClose={() => setOpen(false)} />
      </Toolbar>
    </AppBar>
  );
}
