import { Divider, Drawer, Stack } from "@mui/material";
import { drawerClasses } from "@mui/material/Drawer";
import { X } from "lucide-react";
import MenuButton from "./MenuButton";
import MenuContent from "./MenuContent";
import { Account, Brand } from "./SideMenu";
import { type NavigationProps } from "../Dashboard";

export default function SideMenuMobile({
  open,
  onClose,
  ...props
}: NavigationProps & { open: boolean; onClose: () => void }) {
  return (
    <Drawer
      anchor="right"
      open={open}
      onClose={onClose}
      sx={{
        zIndex: (theme) => theme.zIndex.drawer + 1,
        [`& .${drawerClasses.paper}`]: {
          backgroundImage: "none",
          backgroundColor: "background.paper",
        },
      }}
    >
      <Stack
        component="aside"
        aria-label="Workspace sidebar"
        sx={{ width: 280, maxWidth: "90dvw", height: "100%", minHeight: 0 }}
      >
        <Stack direction="row" sx={{ alignItems: "center", pr: 1 }}>
          <Brand onNavigate={onClose} />
          <MenuButton aria-label="Close navigation" onClick={onClose}>
            <X size={20} />
          </MenuButton>
        </Stack>
        <Divider />
        <MenuContent {...props} onNavigate={onClose} />
        <Account {...props} onNavigate={onClose} />
      </Stack>
    </Drawer>
  );
}
