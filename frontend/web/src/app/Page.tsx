import type { ReactNode } from "react";
import { Box, Button, Container, Stack, Typography } from "@mui/material";
import { ArrowLeft } from "lucide-react";

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
        {back && (
          <Button
            href={back.href}
            startIcon={<ArrowLeft size={18} />}
            sx={{ alignSelf: "flex-start" }}
          >
            {back.label}
          </Button>
        )}
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
                alignItems: "center",
                gap: 1,
                flexWrap: "wrap",
              }}
            >
              {actions}
            </Box>
          )}
        </Stack>
        {children}
      </Stack>
    </Container>
  );
}
