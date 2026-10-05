import { Component, type ReactNode } from "react";
import { Alert, Button, Container } from "@mui/material";

export class ErrorBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <Container maxWidth="sm" sx={{ py: 4 }}>
        <Alert
          severity="error"
          action={
            <Button onClick={() => location.reload()}>Reload console</Button>
          }
        >
          The console could not load. Reload to get the latest version.
        </Alert>
      </Container>
    );
  }
}
