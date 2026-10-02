import {
  Chip,
  Grid,
  IconButton,
  Link,
  List,
  ListItem,
  Stack,
  Tooltip,
  Typography,
} from "@mui/material";
import { Trash2 } from "lucide-react";
import type { Credential } from "../../api/identity";
import { time } from "../../design/format";

export function TokenList({
  tokens,
  onRevoke,
  activityHref,
}: {
  tokens: Credential[];
  onRevoke: (token: Credential) => void;
  activityHref?: (token: Credential) => string;
}) {
  return (
    <List disablePadding aria-label="Management API tokens">
      {tokens.map((token) => {
        const status = token.revoked_at
          ? "Revoked"
          : Date.parse(token.expires_at) <= Date.now()
            ? "Expired"
            : "Active";
        return (
          <ListItem key={token.id} divider sx={{ py: 2 }}>
            <Grid
              container
              spacing={2}
              sx={{ width: "100%", alignItems: "center" }}
            >
              <Grid size={{ xs: 10, md: 5 }}>
                <Typography
                  variant="subtitle2"
                  sx={{ overflowWrap: "anywhere" }}
                >
                  {token.label}
                </Typography>
                <Typography
                  component="code"
                  variant="body2"
                  sx={{ fontFamily: "monospace" }}
                >
                  {token.token_prefix}
                </Typography>
              </Grid>
              <Grid
                size={{ xs: 2, md: 1 }}
                sx={{ order: { md: 3 }, textAlign: "right" }}
              >
                <Tooltip title={`Revoke ${token.label}`}>
                  <span>
                    <IconButton
                      aria-label={`Revoke ${token.label}`}
                      disabled={Boolean(token.revoked_at)}
                      onClick={() => onRevoke(token)}
                    >
                      <Trash2 size={18} />
                    </IconButton>
                  </span>
                </Tooltip>
              </Grid>
              <Grid size={{ xs: 12, md: 6 }}>
                <Stack
                  direction="row"
                  spacing={1}
                  useFlexGap
                  sx={{ alignItems: "center", flexWrap: "wrap" }}
                >
                  <Chip
                    size="small"
                    label={status}
                    color={status === "Active" ? "success" : "default"}
                    variant="outlined"
                  />
                  <Typography variant="body2" color="text.secondary">
                    Expires {time(token.expires_at)}
                  </Typography>
                </Stack>
                {activityHref && (
                  <Link variant="body2" href={activityHref(token)}>
                    View token actions
                  </Link>
                )}
              </Grid>
            </Grid>
          </ListItem>
        );
      })}
    </List>
  );
}
