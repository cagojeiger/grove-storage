import { styled } from "@mui/material/styles";
import Typography from "@mui/material/Typography";
import Link from "@mui/material/Link";
import Breadcrumbs, { breadcrumbsClasses } from "@mui/material/Breadcrumbs";
import NavigateNextRoundedIcon from "@mui/icons-material/NavigateNextRounded";

const StyledBreadcrumbs = styled(Breadcrumbs)(({ theme }) => ({
  margin: theme.spacing(1, 0),
  [`& .${breadcrumbsClasses.separator}`]: {
    color: (theme.vars || theme).palette.action.disabled,
    margin: 1,
  },
  [`& .${breadcrumbsClasses.ol}`]: { alignItems: "center" },
}));
export default function NavbarBreadcrumbs({ route }: { route: string }) {
  const key = route.split("/")[0];
  const label =
    (
      {
        storages: "Storage",
        clients: "Clients",
        accounts: "Accounts",
        activity: "Activity",
        settings: "My account",
        usage: "Usage history",
      } as Record<string, string>
    )[key] ?? "Overview";
  return (
    <StyledBreadcrumbs
      aria-label="Console path"
      separator={<NavigateNextRoundedIcon fontSize="small" />}
    >
      <Link href="#" variant="body1">
        Console
      </Link>
      <Typography
        variant="body1"
        sx={{ color: "text.primary", fontWeight: 600 }}
      >
        {label}
      </Typography>
    </StyledBreadcrumbs>
  );
}
