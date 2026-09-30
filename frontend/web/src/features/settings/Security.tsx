import { ArrowLeft } from "lucide-react";
import { Button, Container, Grid, Stack, Typography } from "@mui/material";
import { PasswordChange } from "./PasswordChange";

export function Security() {
  return (
    <Container component="main" maxWidth="lg" sx={{ py: 3 }}>
      <Stack spacing={3}>
        <Button
          href="#settings"
          startIcon={<ArrowLeft size={16} />}
          sx={{ alignSelf: "flex-start" }}
        >
          My account
        </Button>
        <Typography component="h1" variant="h5">
          Security
        </Typography>
        <Grid container>
          <Grid size={{ xs: 12, md: 6 }}>
            <PasswordChange />
          </Grid>
        </Grid>
      </Stack>
    </Container>
  );
}
