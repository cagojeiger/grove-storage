import { useState, type ReactNode } from "react";
import {
  AppBar,
  Box,
  Breadcrumbs,
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

const drawerWidth = 256;
const sections = [
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
  const section =
    root === "usage"
      ? sections[0]
      : root === "access"
        ? sections[3]
        : sections.find((item) => item.path === root);
  const label =
    root === "usage"
      ? "Usage history"
      : route === "settings/security"
        ? "Security"
        : (section?.label ?? "My account");
  const brand = (
    <Stack
      component="a"
      href={import.meta.env.BASE_URL}
      direction="row"
      spacing={1.5}
      sx={{
        alignItems: "center",
        color: "text.primary",
        textDecoration: "none",
      }}
    >
      <Box
        component="img"
        src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
        alt=""
        sx={{ width: 36, height: 36 }}
      />
      <Typography component="span" variant="subtitle1">
        Grove Storage
      </Typography>
    </Stack>
  );

  return (
    <Box sx={{ display: "flex", minHeight: "100dvh", minWidth: 0 }}>
      {session && (
        <Drawer
          variant={desktop ? "permanent" : "temporary"}
          open={desktop || open}
          onClose={close}
          sx={{ width: desktop ? drawerWidth : 0, flexShrink: 0 }}
          slotProps={{ paper: { sx: { width: drawerWidth } } }}
        >
          <Stack
            component="aside"
            aria-label="Workspace sidebar"
            sx={{ height: "100%", minHeight: 0 }}
          >
            <Toolbar sx={{ justifyContent: "space-between", gap: 1 }}>
              {brand}
              {!desktop && (
                <Tooltip title="Close navigation">
                  <IconButton aria-label="Close navigation" onClick={close}>
                    <X size={20} />
                  </IconButton>
                </Tooltip>
              )}
            </Toolbar>
            <Divider />
            <List
              component="nav"
              aria-label="Main navigation"
              sx={{ px: 1, flex: 1, overflowY: "auto" }}
            >
              {sections
                .filter(
                  (item) =>
                    item.path !== "accounts" || session.role === "admin",
                )
                .map((item) => {
                  const selected = section?.path === item.path;
                  const group =
                    item.path === "activity" && session.role !== "admin"
                      ? "Management"
                      : item.group;
                  const Icon = item.icon;
                  return (
                    <Box key={item.path}>
                      {group && (
                        <ListSubheader disableSticky>{group}</ListSubheader>
                      )}
                      <ListItemButton
                        component="a"
                        href={`#${item.path}`}
                        selected={selected}
                        aria-current={selected ? "page" : undefined}
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
            <Box sx={{ p: 1 }}>
              <AccountMenu
                name={name}
                role={session.role}
                passwordSession={session.credential_id === null}
                loggingOut={loggingOut}
                onLogout={onLogout}
                onNavigate={close}
              />
            </Box>
          </Stack>
        </Drawer>
      )}
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <AppBar
          position="sticky"
          color="inherit"
          elevation={0}
          sx={{ borderBottom: 1, borderColor: "divider" }}
        >
          <Toolbar sx={{ gap: 2 }}>
            {session && !desktop && (
              <Tooltip title="Open navigation">
                <IconButton
                  edge="start"
                  aria-label="Open navigation"
                  onClick={() => setOpen(true)}
                >
                  <Menu size={20} />
                </IconButton>
              </Tooltip>
            )}
            {session ? (
              <Breadcrumbs
                aria-label="Breadcrumb"
                sx={{ minWidth: 0, flex: 1 }}
              >
                <Link color="inherit" underline="hover" href="#">
                  Console
                </Link>
                <Typography color="text.primary">{label}</Typography>
              </Breadcrumbs>
            ) : (
              <Box sx={{ flex: 1 }}>{brand}</Box>
            )}
            <ThemePicker />
          </Toolbar>
        </AppBar>
        {children}
      </Box>
    </Box>
  );
}
