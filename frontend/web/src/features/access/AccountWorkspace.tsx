import { Button, Chip, Divider, Stack, Typography } from "@mui/material";
import { Shield, Trash2 } from "lucide-react";
import type { Account } from "../../api/identity";
import { DetailSections } from "../../app/DetailSections";
import { Details } from "../../app/Details";
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
    <DetailSections
      label="Account sections"
      sections={[
        {
          value: "overview",
          label: "Overview",
          content: (
            <Stack spacing={3}>
              <Stack
                direction="row"
                spacing={1}
                useFlexGap
                sx={{ flexWrap: "wrap", alignItems: "center" }}
              >
                <Typography component="h2" variant="h6">
                  Account details
                </Typography>
              </Stack>
              <Details
                label="Account details"
                items={[
                  ["Username", account.username ?? "Not set"],
                  ["Role", account.role],
                  [
                    "Status",
                    <Chip
                      size="small"
                      label={status}
                      color={status === "Active" ? "success" : "default"}
                    />,
                  ],
                  ["Account ID", account.id],
                ]}
              />
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
            <Stack spacing={4} divider={<Divider />}>
              <PasswordSetupIssue key={account.id} account={account} />
              <Stack
                component="section"
                aria-label="Account permissions"
                spacing={2}
              >
                <Typography component="h2" variant="h6">
                  Permissions
                </Typography>
                <Details
                  label="Permissions"
                  items={[
                    ["Role", account.role],
                    ["Status", status],
                  ]}
                />
                <Button
                  variant="outlined"
                  startIcon={<Shield size={18} />}
                  disabled={disabled}
                  sx={{ alignSelf: "flex-start" }}
                  onClick={() => onAction("role")}
                >
                  Change role
                </Button>
              </Stack>
              <Stack component="section" aria-label="Danger zone" spacing={2}>
                <Typography component="h2" variant="h6">
                  Danger zone
                </Typography>
                <Stack direction={{ xs: "column", sm: "row" }} spacing={2}>
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
                    startIcon={<Trash2 size={18} />}
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
  );
}
