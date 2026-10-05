import {
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Stack,
} from "@mui/material";
import {
  Activity,
  Database,
  HardDrive,
  LayoutDashboard,
  Users,
} from "lucide-react";
import { type NavigationProps } from "../Dashboard";

export default function MenuContent({
  session,
  route,
  onNavigate,
}: Pick<NavigationProps, "session" | "route"> & { onNavigate?: () => void }) {
  const groups = [
    {
      label: "",
      items: [
        {
          text: "Overview",
          href: "#",
          icon: LayoutDashboard,
          selected: !route || route === "usage",
        },
      ],
    },
    {
      label: "Resources",
      items: [
        {
          text: "Storage",
          href: "#storages",
          icon: HardDrive,
          selected: route.startsWith("storages"),
        },
        {
          text: "Clients",
          href: "#clients",
          icon: Database,
          selected: route.startsWith("clients"),
        },
      ],
    },
    {
      label: "Management",
      items: [
        ...(session.role === "admin"
          ? [
              {
                text: "Accounts",
                href: "#accounts",
                icon: Users,
                selected: route.startsWith("accounts"),
              },
            ]
          : []),
        {
          text: session.role === "admin" ? "Activity" : "My activity",
          href: "#activity",
          icon: Activity,
          selected: route.startsWith("activity"),
        },
      ],
    },
  ];
  return (
    <Stack
      component="nav"
      aria-label="Main navigation"
      sx={{ flexGrow: 1, p: 1, overflowY: "auto" }}
    >
      {groups.map((group) => (
        <List
          dense
          key={group.label}
          subheader={
            group.label ? (
              <ListSubheader>{group.label}</ListSubheader>
            ) : undefined
          }
        >
          {group.items.map((item) => (
            <ListItem key={item.href} disablePadding sx={{ display: "block" }}>
              <ListItemButton
                component="a"
                href={item.href}
                selected={item.selected}
                aria-current={item.selected ? "page" : undefined}
                onClick={onNavigate}
              >
                <ListItemIcon>
                  <item.icon size={20} />
                </ListItemIcon>
                <ListItemText primary={item.text} />
              </ListItemButton>
            </ListItem>
          ))}
        </List>
      ))}
    </Stack>
  );
}
