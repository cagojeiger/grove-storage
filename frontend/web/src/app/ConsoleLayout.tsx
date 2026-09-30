import { useId, useState, type ReactNode } from "react";
import {
  AppBar,
  Box,
  Button,
  Divider,
  Drawer,
  IconButton,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Menu,
  MenuItem,
  Stack,
  Toolbar,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";
import {
  AppWindow,
  ChevronUp,
  CircleUserRound,
  HardDrive,
  LayoutDashboard,
  LogOut,
  Menu as MenuIcon,
  ScrollText,
  Settings,
  Shield,
  X,
} from "lucide-react";
import type { Session } from "../api/http";
import { ThemePicker } from "../design/Theme";

const headerHeight = 56;
const drawerWidth = 240;

export function ConsoleLayout({
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
  const theme = useTheme();
  const desktop = useMediaQuery(theme.breakpoints.up("md"));
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);
  const navigation = [
    {
      label: "Overview",
      href: "#",
      icon: LayoutDashboard,
      selected: route === "" || route === "usage",
    },
    {
      group: "Resources",
      label: "Storage",
      href: "#storages",
      icon: HardDrive,
      selected: route.startsWith("storages"),
    },
    {
      label: "Clients",
      href: "#clients",
      icon: AppWindow,
      selected: route.startsWith("clients"),
    },
    ...(session?.role === "admin"
      ? [
          {
            group: "Management",
            label: "Accounts",
            href: "#accounts",
            icon: Shield,
            selected:
              route.startsWith("accounts") || route.startsWith("access"),
          },
        ]
      : []),
    {
      group: session?.role === "admin" ? undefined : "Management",
      label: "Activity",
      href: "#activity",
      icon: ScrollText,
      selected: route.startsWith("activity"),
    },
  ];
  return (
    <>
      <AppBar
        position="sticky"
        color="inherit"
        elevation={0}
        sx={{ borderBottom: 1, borderColor: "divider" }}
      >
        <Toolbar variant="dense" sx={{ gap: 1.5, minHeight: headerHeight }}>
          {!desktop && session && (
            <Tooltip title="Open navigation">
              <IconButton
                aria-label="Open navigation"
                onClick={() => setOpen(true)}
              >
                <MenuIcon size={20} />
              </IconButton>
            </Tooltip>
          )}
          <Stack
            component="a"
            href={import.meta.env.BASE_URL}
            direction="row"
            spacing={1.25}
            sx={{
              alignItems: "center",
              color: "text.primary",
              textDecoration: "none",
              minWidth: 0,
            }}
          >
            <Box
              component="img"
              src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
              alt=""
              sx={{ width: 32, height: 32 }}
            />
            <Typography
              component="span"
              variant="subtitle1"
              sx={{ fontWeight: 600 }}
            >
              Grove Storage
            </Typography>
          </Stack>
          <Box sx={{ ml: "auto" }}>
            <ThemePicker />
          </Box>
        </Toolbar>
      </AppBar>
      <Box sx={{ display: "flex", flex: 1, minWidth: 0 }}>
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
                  top: desktop ? headerHeight : 0,
                  height: desktop
                    ? `calc(100dvh - ${headerHeight}px)`
                    : "100dvh",
                },
              },
            }}
          >
            <Stack
              component="aside"
              aria-label="Workspace sidebar"
              sx={{ p: 2, height: "100%", minHeight: 0 }}
            >
              {!desktop && (
                <Stack
                  direction="row"
                  sx={{ alignItems: "center", justifyContent: "space-between" }}
                >
                  <Typography sx={{ fontWeight: 600 }}>Navigation</Typography>
                  <Tooltip title="Close navigation">
                    <IconButton aria-label="Close navigation" onClick={close}>
                      <X size={18} />
                    </IconButton>
                  </Tooltip>
                </Stack>
              )}
              <List
                component="nav"
                aria-label="Main navigation"
                sx={{ overflowY: "auto", minHeight: 0 }}
              >
                {navigation.map(
                  ({ group, label, href, icon: Icon, selected }) => (
                    <Box key={href}>
                      {group && (
                        <ListSubheader
                          component="div"
                          disableSticky
                          sx={{ bgcolor: "transparent", mt: 1 }}
                        >
                          {group}
                        </ListSubheader>
                      )}
                      <ListItemButton
                        component="a"
                        href={href}
                        selected={selected}
                        aria-current={selected ? "page" : undefined}
                        onClick={close}
                      >
                        <ListItemIcon sx={{ minWidth: 32 }}>
                          <Icon size={18} />
                        </ListItemIcon>
                        <ListItemText primary={label} />
                      </ListItemButton>
                    </Box>
                  ),
                )}
              </List>
              <Box
                sx={{ mt: "auto", pt: 2, borderTop: 1, borderColor: "divider" }}
              >
                <Typography
                  variant="caption"
                  color="text.secondary"
                  sx={{ px: 1 }}
                >
                  {
                    {
                      reader: "Reader · Read-only",
                      writer: "Writer · Operations",
                      admin: "Admin · Management",
                    }[session.role]
                  }
                </Typography>
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
        <Box sx={{ flex: 1, minWidth: 0 }}>{children}</Box>
      </Box>
    </>
  );
}

function AccountMenu({
  name,
  role,
  passwordSession,
  loggingOut,
  onLogout,
  onNavigate,
}: {
  name: string;
  role: string;
  passwordSession: boolean;
  loggingOut: boolean;
  onLogout: () => void;
  onNavigate: () => void;
}) {
  const id = useId();
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const close = () => setAnchor(null);
  const navigate = () => {
    close();
    onNavigate();
  };
  return (
    <>
      <Button
        id={id}
        fullWidth
        variant="text"
        aria-label="Account menu"
        aria-haspopup="menu"
        aria-controls={anchor ? `${id}-menu` : undefined}
        aria-expanded={Boolean(anchor)}
        onClick={(event) => setAnchor(event.currentTarget)}
        startIcon={<CircleUserRound size={18} />}
        endIcon={<ChevronUp size={16} />}
        sx={{
          justifyContent: "flex-start",
          color: "text.primary",
          minHeight: 44,
        }}
      >
        <Typography
          component="span"
          variant="body2"
          noWrap
          sx={{ flex: 1, textAlign: "left" }}
        >
          {name}
        </Typography>
      </Button>
      <Menu
        anchorEl={anchor}
        open={Boolean(anchor)}
        onClose={close}
        anchorOrigin={{ vertical: "top", horizontal: "left" }}
        transformOrigin={{ vertical: "bottom", horizontal: "left" }}
        slotProps={{
          list: { id: `${id}-menu`, "aria-labelledby": id },
          paper: { sx: { width: drawerWidth, maxWidth: "calc(100vw - 32px)" } },
        }}
      >
        <ListSubheader
          sx={{
            lineHeight: 1.5,
            py: 1,
            whiteSpace: "normal",
            overflowWrap: "anywhere",
          }}
        >
          <Typography variant="subtitle2">{name}</Typography>
          <Typography variant="caption" sx={{ textTransform: "capitalize" }}>
            {role}
          </Typography>
        </ListSubheader>
        <Divider />
        <MenuItem component="a" href="#settings" onClick={navigate}>
          <ListItemIcon>
            <CircleUserRound size={18} />
          </ListItemIcon>
          My account
        </MenuItem>
        {passwordSession && (
          <MenuItem component="a" href="#settings/security" onClick={navigate}>
            <ListItemIcon>
              <Settings size={18} />
            </ListItemIcon>
            Security
          </MenuItem>
        )}
        <MenuItem
          disabled={loggingOut}
          onClick={() => {
            navigate();
            onLogout();
          }}
        >
          <ListItemIcon>
            <LogOut size={18} />
          </ListItemIcon>
          Sign out
        </MenuItem>
      </Menu>
    </>
  );
}
