import {
  Box,
  Button,
  Chip,
  Divider,
  Grid,
  Stack,
  Tab,
  Tabs,
  Typography,
} from "@mui/material";
import { Shield, Trash2 } from "lucide-react";
import type { Account } from "../../api/identity";
import { useRoute } from "../../app/navigation";
import type { AccountAction } from "./AccountDialog";
import { useAccountList } from "./accountList";
import { PasswordSetupIssue } from "./PasswordSetupIssue";
import { Tokens } from "./Tokens";

const tabs = ["overview", "tokens", "security"] as const;
const labels = ["Overview", "API tokens", "Security"];

export function AccountWorkspace({
  account,
  onAction,
}: {
  account: Account;
  onAction: (action: AccountAction) => void;
}) {
  const route = useRoute();
  const listing = useAccountList();
  const requested = new URLSearchParams(route.split("?")[1]).get("tab");
  const selected = tabs.find((tab) => tab === requested) ?? "overview";
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
      <Tabs
        value={selected}
        aria-label="Account sections"
        variant="scrollable"
        scrollButtons="auto"
        sx={{ borderBottom: 1, borderColor: "divider" }}
      >
        {tabs.map((tab, index) => (
          <Tab
            key={tab}
            value={tab}
            label={labels[index]}
            id={`account-tab-${tab}`}
            aria-controls={`account-panel-${tab}`}
            component="a"
            href={`${listing.href(`#accounts/${encodeURIComponent(account.id)}`)}&tab=${tab}`}
          />
        ))}
      </Tabs>
      <Box
        role="tabpanel"
        id={`account-panel-${selected}`}
        aria-labelledby={`account-tab-${selected}`}
      >
        {selected === "overview" && (
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
        )}
        {selected === "tokens" && <Tokens key={account.id} account={account} />}
        {selected === "security" && (
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
                sx={{ alignItems: "center", justifyContent: "space-between" }}
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
        )}
      </Box>
    </Stack>
  );
}
