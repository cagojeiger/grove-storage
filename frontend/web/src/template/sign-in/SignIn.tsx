// Adapted from MUI's Sign-in and shared-theme templates; see MUI-TEMPLATE-LICENSE.txt.
import { type ReactNode } from "react";
import { Box, Card, Stack, Typography, styled } from "@mui/material";
import ThemeMode from "../shared-theme/ColorModeIconDropdown";

const SignInContainer = styled(Stack)(({ theme }) => ({
  minHeight: "100dvh",
  padding: theme.spacing(2),
  gap: theme.spacing(3),
  backgroundColor: "#f4f6f5",
  [theme.breakpoints.up("sm")]: { padding: theme.spacing(4) },
  ...theme.applyStyles("dark", { backgroundColor: "#101412" }),
}));

const SignInCard = styled(Card)<{ component?: "main" }>(({ theme }) => ({
  display: "flex",
  flexDirection: "column",
  alignSelf: "center",
  width: "100%",
  maxWidth: 450,
  padding: theme.spacing(4),
  gap: theme.spacing(2),
  margin: "auto",
  overflow: "visible",
  boxShadow:
    "hsla(220, 30%, 5%, 0.05) 0px 5px 15px 0px, hsla(220, 25%, 10%, 0.05) 0px 15px 35px -5px",
  ...theme.applyStyles("dark", {
    boxShadow:
      "hsla(220, 30%, 5%, 0.5) 0px 5px 15px 0px, hsla(220, 25%, 10%, 0.08) 0px 15px 35px -5px",
  }),
}));

export default function SignIn({ children }: { children: ReactNode }) {
  return (
    <SignInContainer>
      <Stack
        component="header"
        direction="row"
        sx={{ justifyContent: "flex-end" }}
      >
        <ThemeMode />
      </Stack>
      <SignInCard component="main" variant="outlined">
        <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
          <Box
            component="img"
            src={`${import.meta.env.BASE_URL}grove-storage-logo.png`}
            alt=""
            sx={{ width: 32, height: 32 }}
          />
          <Typography variant="h6">Grove Storage</Typography>
        </Stack>
        {children}
      </SignInCard>
      <Typography
        component="footer"
        variant="caption"
        color="text.secondary"
        sx={{ textAlign: "center" }}
      >
        Grove Storage
      </Typography>
    </SignInContainer>
  );
}
