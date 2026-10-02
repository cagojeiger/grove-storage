import type { ReactNode } from "react";
import {
  Box,
  Breadcrumbs,
  Container,
  Divider,
  Link,
  Stack,
  Typography,
} from "@mui/material";

export function Page({
  title,
  back,
  actions,
  children,
}: {
  title: string;
  back?: { label: string; href: string };
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Container component="main" maxWidth="xl" sx={{ py: 3 }}>
      <Stack spacing={3}>
        <Breadcrumbs aria-label="Breadcrumb" sx={{ overflowWrap: "anywhere" }}>
          <Link color="inherit" href="#">
            Console
          </Link>
          {back && (
            <Link color="inherit" href={back.href}>
              {back.label}
            </Link>
          )}
          <Typography color="text.primary">{title}</Typography>
        </Breadcrumbs>
        <Stack
          direction={{ xs: "column", sm: "row" }}
          spacing={2}
          sx={{ justifyContent: "space-between", alignItems: { sm: "center" } }}
        >
          <Typography
            component="h1"
            variant="h5"
            sx={{ minWidth: 0, overflowWrap: "anywhere" }}
          >
            {title}
          </Typography>
          {actions && (
            <Box
              sx={{
                display: "flex",
                gap: 1,
                alignItems: "center",
                flexWrap: "wrap",
              }}
            >
              {actions}
            </Box>
          )}
        </Stack>
        <Divider />
        {children}
      </Stack>
    </Container>
  );
}
