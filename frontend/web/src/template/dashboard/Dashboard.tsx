import { type ReactNode } from "react";
import Box from "@mui/material/Box";
import Stack from "@mui/material/Stack";
import Typography from "@mui/material/Typography";
import SideMenu from "./components/SideMenu";
import AppNavbar from "./components/AppNavbar";
import Header from "./components/Header";
import { type Session } from "../../api/http";

export interface NavigationProps {
  session: Session;
  route: string;
  name: string;
  onLogout: () => void;
  loggingOut: boolean;
}
export default function Dashboard({
  children,
  ...navigation
}: NavigationProps & { children: ReactNode }) {
  return (
    <Box sx={{ display: "flex", minHeight: "100dvh" }}>
      <SideMenu {...navigation} />
      <AppNavbar {...navigation} />
      <Box
        component="main"
        sx={{ flexGrow: 1, minWidth: 0, backgroundColor: "background.default" }}
      >
        <Stack
          spacing={2}
          sx={{
            alignItems: "center",
            mx: { xs: 2, sm: 3 },
            pb: 3,
            pt: { xs: 8, md: 0 },
            height: "100dvh",
            overflowY: "auto",
          }}
        >
          <Header route={navigation.route} />
          <Box
            sx={{
              width: "100%",
              maxWidth: 1700,
              minWidth: 0,
              display: "flex",
              flex: "1 0 auto",
            }}
          >
            {children}
          </Box>
          <Typography
            component="footer"
            role="contentinfo"
            variant="caption"
            color="text.secondary"
            sx={{ width: "100%", maxWidth: 1700, pt: 2 }}
          >
            Grove Storage · S3 gateway
          </Typography>
        </Stack>
      </Box>
    </Box>
  );
}
