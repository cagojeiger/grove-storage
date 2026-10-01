import { Box } from "@mui/material";
import { Page } from "../../app/Page";
import { PasswordChange } from "./PasswordChange";

export function Security() {
  return (
    <Page title="Security" back={{ label: "My account", href: "#settings" }}>
      <Box sx={{ maxWidth: 480 }}>
        <PasswordChange />
      </Box>
    </Page>
  );
}
