import { styled } from "@mui/material/styles";
import MuiDrawer, { drawerClasses } from "@mui/material/Drawer";
import {
  Avatar,
  Box,
  Divider,
  IconButton,
  Link,
  ListItemButton,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import LogoutRoundedIcon from "@mui/icons-material/LogoutRounded";
import MenuContent from "./MenuContent";
import { type NavigationProps } from "../Dashboard";

const drawerWidth = 240;
const Drawer = styled(MuiDrawer)({
  width: drawerWidth,
  flexShrink: 0,
  boxSizing: "border-box",
  [`& .${drawerClasses.paper}`]: {
    width: drawerWidth,
    boxSizing: "border-box",
  },
});
export function Brand({ onNavigate }: { onNavigate?: () => void }) {
  return (
    <Stack direction="row" spacing={1} sx={{ p: 2, alignItems: "center" }}>
      <Box
        component="img"
        src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
        alt=""
        sx={{ width: 32, height: 32 }}
      />
      <Link
        href="#"
        onClick={onNavigate}
        variant="subtitle2"
        color="text.primary"
      >
        Grove Storage
      </Link>
    </Stack>
  );
}
export function Account({
  name,
  session,
  onLogout,
  loggingOut,
  onNavigate,
}: NavigationProps & { onNavigate?: () => void }) {
  return (
    <Stack
      direction="row"
      sx={{
        p: 1,
        gap: 1,
        alignItems: "center",
        borderTop: 1,
        borderColor: "divider",
      }}
    >
      <ListItemButton
        component="a"
        href="#settings"
        aria-label="My account"
        onClick={onNavigate}
        sx={{ minWidth: 0, gap: 1, px: 1 }}
      >
        <Avatar sx={{ width: 36, height: 36 }}>{name[0]}</Avatar>
        <Box sx={{ minWidth: 0 }}>
          <Typography
            variant="body2"
            noWrap
            title={name}
            sx={{ fontWeight: 500 }}
          >
            {name}
          </Typography>
          <Typography variant="caption" color="text.secondary">
            {session.role}
          </Typography>
        </Box>
      </ListItemButton>
      <Tooltip title="Sign out">
        <span>
          <IconButton
            aria-label="Sign out"
            disabled={loggingOut}
            onClick={() => {
              onNavigate?.();
              onLogout();
            }}
          >
            <LogoutRoundedIcon fontSize="small" />
          </IconButton>
        </span>
      </Tooltip>
    </Stack>
  );
}
export default function SideMenu(props: NavigationProps) {
  return (
    <Drawer
      variant="permanent"
      sx={{
        display: { xs: "none", md: "block" },
        [`& .${drawerClasses.paper}`]: { backgroundColor: "background.paper" },
      }}
    >
      <Stack
        component="aside"
        aria-label="Workspace sidebar"
        sx={{ height: "100%", minHeight: 0 }}
      >
        <Brand />
        <Divider />
        <MenuContent {...props} />
        <Account {...props} />
      </Stack>
    </Drawer>
  );
}
