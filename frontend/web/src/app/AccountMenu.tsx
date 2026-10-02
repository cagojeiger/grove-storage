import { useId, useState } from "react";
import {
  Avatar,
  Divider,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Menu,
  MenuItem,
  Typography,
} from "@mui/material";
import { ChevronUp, CircleUserRound, LogOut, Shield } from "lucide-react";

export function AccountMenu({
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
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  function close() {
    setAnchor(null);
  }
  function navigate() {
    close();
    onNavigate();
  }
  return (
    <>
      <ListItemButton
        component="button"
        aria-label="Account menu"
        aria-haspopup="menu"
        aria-expanded={Boolean(anchor)}
        aria-controls={anchor ? id : undefined}
        onClick={(event) => setAnchor(event.currentTarget)}
        sx={{ width: "100%", gap: 1.5, flexGrow: 0, flexShrink: 0 }}
      >
        <Avatar>
          <CircleUserRound size={22} />
        </Avatar>
        <ListItemText
          primary={name}
          secondary={role}
          slotProps={{
            primary: { noWrap: true },
            secondary: { sx: { textTransform: "capitalize" } },
          }}
        />
        <ChevronUp size={18} />
      </ListItemButton>
      <Menu
        anchorEl={anchor}
        open={Boolean(anchor)}
        onClose={close}
        anchorOrigin={{ vertical: "top", horizontal: "left" }}
        transformOrigin={{ vertical: "bottom", horizontal: "left" }}
        slotProps={{
          list: { id, "aria-label": "Account menu" },
        }}
      >
        <ListSubheader
          component="div"
          disableSticky
          sx={{ py: 1, maxWidth: 280, overflowWrap: "anywhere" }}
        >
          <Typography variant="subtitle2">{name}</Typography>
          <Typography
            variant="body2"
            color="text.secondary"
            sx={{ textTransform: "capitalize" }}
          >
            {role}
          </Typography>
        </ListSubheader>
        <Divider />
        <MenuItem component="a" href="#settings" onClick={navigate}>
          <ListItemIcon>
            <CircleUserRound size={20} />
          </ListItemIcon>
          My account
        </MenuItem>
        {passwordSession && (
          <MenuItem component="a" href="#settings/security" onClick={navigate}>
            <ListItemIcon>
              <Shield size={20} />
            </ListItemIcon>
            Security
          </MenuItem>
        )}
        <Divider />
        <MenuItem
          disabled={loggingOut}
          onClick={() => {
            navigate();
            onLogout();
          }}
        >
          <ListItemIcon>
            <LogOut size={20} />
          </ListItemIcon>
          Sign out
        </MenuItem>
      </Menu>
    </>
  );
}
