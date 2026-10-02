import { useState, type ReactNode } from "react";
import {
  AppBar,
  Box,
  Divider,
  Drawer,
  IconButton,
  Link,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Stack,
  Toolbar,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";
import {
  AppWindow,
  HardDrive,
  LayoutDashboard,
  Menu,
  ScrollText,
  Shield,
  X,
} from "lucide-react";
import type { Session } from "../api/http";
import { ThemePicker } from "../design/Theme";
import { AccountMenu } from "./AccountMenu";

const drawerWidth = 232;
const destinations = [
  { label: "Overview", path: "", icon: LayoutDashboard },
  { label: "Storage", path: "storages", icon: HardDrive, group: "Resources" },
  { label: "Clients", path: "clients", icon: AppWindow },
  { label: "Accounts", path: "accounts", icon: Shield, group: "Management" },
  { label: "Activity", path: "activity", icon: ScrollText },
];

export function Shell({
  session,
  route,
  name,
  loggingOut,
  onLogout,
  children,
}: {
  session: Session | null | undefined;
  route: string;
  name: string;
  loggingOut: boolean;
  onLogout: () => void;
  children: ReactNode;
}) {
  const desktop = useMediaQuery(useTheme().breakpoints.up("md"));
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);
  const root = route.split("/")[0];
  const selected =
    root === "usage" ? "" : root === "access" ? "accounts" : root;
  return (
    <Box sx={{ display: "flex", minHeight: "100dvh" }}>
      <AppBar
        position="fixed"
        sx={(theme) => ({
          zIndex: desktop ? theme.zIndex.drawer + 1 : theme.zIndex.appBar,
        })}
      >
        <Toolbar sx={{ gap: 2 }}>
          {session && !desktop && (
            <Tooltip title="Open navigation">
              <IconButton
                color="inherit"
                edge="start"
                aria-label="Open navigation"
                onClick={() => setOpen(true)}
              >
                <Menu />
              </IconButton>
            </Tooltip>
          )}
          <Link
            href={import.meta.env.BASE_URL}
            color="inherit"
            underline="none"
            sx={{
              display: "flex",
              alignItems: "center",
              gap: 1,
              flex: 1,
              minWidth: 0,
            }}
          >
            <Box
              component="img"
              src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
              alt=""
              sx={{ width: 32, height: 32 }}
            />
            <Typography component="span" variant="h6" noWrap>
              Grove Storage
            </Typography>
          </Link>
          <ThemePicker />
        </Toolbar>
      </AppBar>
      {session && (
        <Drawer
          variant={desktop ? "permanent" : "temporary"}
          open={desktop || open}
          onClose={close}
          sx={{ width: desktop ? drawerWidth : 0, flexShrink: 0 }}
          slotProps={{
            paper: {
              sx: {
                width: drawerWidth,
                ...(desktop ? { top: 64, height: "calc(100% - 64px)" } : {}),
              },
            },
          }}
        >
          <Stack
            component="aside"
            aria-label="Workspace sidebar"
            sx={{ height: "100%", minHeight: 0 }}
          >
            {!desktop && (
              <Toolbar sx={{ justifyContent: "space-between" }}>
                <Typography variant="h6">Console</Typography>
                <IconButton aria-label="Close navigation" onClick={close}>
                  <X />
                </IconButton>
              </Toolbar>
            )}
            <List
              component="nav"
              aria-label="Main navigation"
              sx={{ flex: 1, overflowY: "auto" }}
            >
              {destinations
                .filter(
                  (item) =>
                    item.path !== "accounts" || session.role === "admin",
                )
                .map((item) => {
                  const Icon = item.icon;
                  const group =
                    item.path === "activity" && session.role !== "admin"
                      ? "Management"
                      : item.group;
                  return (
                    <Box key={item.path}>
                      {group && (
                        <ListSubheader disableSticky>{group}</ListSubheader>
                      )}
                      <ListItemButton
                        component="a"
                        href={`#${item.path}`}
                        selected={selected === item.path}
                        aria-current={
                          selected === item.path ? "page" : undefined
                        }
                        onClick={close}
                      >
                        <ListItemIcon>
                          <Icon size={20} />
                        </ListItemIcon>
                        <ListItemText primary={item.label} />
                      </ListItemButton>
                    </Box>
                  );
                })}
            </List>
            <Divider />
            <AccountMenu
              name={name}
              role={session.role}
              passwordSession={session.credential_id === null}
              loggingOut={loggingOut}
              onLogout={onLogout}
              onNavigate={close}
            />
          </Stack>
        </Drawer>
      )}
      <Stack sx={{ flex: 1, minWidth: 0 }}>
        <Toolbar />
        <Box sx={{ flex: 1 }}>{children}</Box>
        <Box
          component="footer"
          sx={{ px: 3, py: 2, borderTop: 1, borderColor: "divider" }}
        >
          <Typography variant="caption" color="text.secondary">
            Grove Storage
          </Typography>
        </Box>
      </Stack>
    </Box>
  );
}
