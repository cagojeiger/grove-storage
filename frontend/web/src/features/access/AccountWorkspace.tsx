import { Button, Chip, Divider, Grid, Stack, Typography } from "@mui/material";
import { Shield, Trash2 } from "lucide-react";
import type { Account } from "../../api/identity";
import { DetailSections } from "../../app/DetailSections";
import type { AccountAction } from "./AccountDialog";
import { PasswordSetupIssue } from "./PasswordSetupIssue";
import { Tokens } from "./Tokens";

export function AccountWorkspace({
  account,
  onAction,
}: {
  account: Account;
  onAction: (action: AccountAction) => void;
}) {
  const disabled = Boolean(account.deleted_at);
  const status = disabled
    ? "Deleted"
    : !account.is_active
      ? "Disabled"
      : account.password_ready
        ? "Active"
        : "Pending setup";

  return (
    <Stack spacing={3}>
      <Stack
        direction="row"
        spacing={1}
        useFlexGap
        sx={{ alignItems: "center", flexWrap: "wrap" }}
      >
        <Typography color="text.secondary">
          {account.username ?? "Username not set"}
        </Typography>
        <Chip size="small" label={account.role} variant="outlined" />
        <Chip
          size="small"
          label={status}
          color={status === "Active" ? "success" : "default"}
          variant="outlined"
        />
      </Stack>
      <DetailSections
        label="Account sections"
        sections={[
          {
            value: "overview",
            label: "Overview",
            content: (
              <Stack spacing={3}>
                <Typography component="h2" variant="h6">
                  Account details
                </Typography>
                <Grid
                  container
                  component="dl"
                  aria-label="Account details"
                  spacing={3}
                  sx={{ m: 0 }}
                >
                  {[
                    ["Account ID", account.id],
                    ["Role", account.role],
                    ["Username", account.username ?? "Not set"],
                    ["Status", status],
                  ].map(([label, value]) => (
                    <Grid key={label} size={{ xs: 12, sm: 6 }}>
                      <Typography
                        component="dt"
                        variant="body2"
                        color="text.secondary"
                      >
                        {label}
                      </Typography>
                      <Typography
                        component="dd"
                        sx={{ m: 0, overflowWrap: "anywhere" }}
                      >
                        {value}
                      </Typography>
                    </Grid>
                  ))}
                </Grid>
              </Stack>
            ),
          },
          {
            value: "tokens",
            label: "API tokens",
            content: <Tokens key={account.id} account={account} />,
          },
          {
            value: "security",
            label: "Security",
            content: (
              <Stack spacing={3} divider={<Divider />}>
                <PasswordSetupIssue key={account.id} account={account} />
                <Stack
                  spacing={2}
                  component="section"
                  aria-label="Account permissions"
                >
                  <Typography component="h2" variant="h6">
                    Permissions
                  </Typography>
                  <Stack
                    direction="row"
                    sx={{
                      alignItems: "center",
                      justifyContent: "space-between",
                    }}
                    spacing={2}
                  >
                    <Typography sx={{ textTransform: "capitalize" }}>
                      {account.role}
                    </Typography>
                    <Button
                      variant="outlined"
                      startIcon={<Shield size={16} />}
                      disabled={disabled}
                      onClick={() => onAction("role")}
                    >
                      Change role
                    </Button>
                  </Stack>
                </Stack>
                <Stack component="section" aria-label="Danger zone" spacing={2}>
                  <Typography component="h2" variant="h6">
                    Danger zone
                  </Typography>
                  <Stack
                    direction="row"
                    spacing={1}
                    useFlexGap
                    sx={{ flexWrap: "wrap" }}
                  >
                    <Button
                      color={account.is_active ? "error" : "primary"}
                      variant="outlined"
                      disabled={disabled}
                      onClick={() => onAction("active")}
                    >
                      {account.is_active ? "Disable" : "Enable"}
                    </Button>
                    <Button
                      color="error"
                      variant="outlined"
                      startIcon={<Trash2 size={16} />}
                      disabled={disabled}
                      onClick={() => onAction("delete")}
                    >
                      Delete account
                    </Button>
                  </Stack>
                </Stack>
              </Stack>
            ),
          },
        ]}
      />
    </Stack>
  );
}
