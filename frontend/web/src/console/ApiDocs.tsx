import { useState } from "react";
import { createPortal } from "react-dom";
import { Box, Link, Paper, Tab, Tabs, useTheme } from "@mui/material";
import { useQuery } from "@tanstack/react-query";
import SwaggerUI from "swagger-ui-react";
import swaggerCss from "swagger-ui-react/swagger-ui.css?inline";
import { cspNonce } from "../app/csp";
import { ApiError, send } from "../api/http";
import { Page, QueryState } from "./ui";

const surfaces = [
  { id: "s3", label: "S3 API" },
  { id: "management", label: "Management API" },
  { id: "native", label: "Native compatibility" },
] as const;
const readOnlyPlugins = [{ components: { authorizeBtn: () => null } }];

export function ApiDocs() {
  const [selected, setSelected] = useState("s3");
  const url = `/api/docs/${selected}.json`;
  const query = useQuery({
    queryKey: ["openapi", selected],
    queryFn: async ({ signal }) => {
      const response = await send(url, { signal });
      if (!response.ok) throw new ApiError(response.status);
      const spec: unknown = await response.json();
      if (!spec || typeof spec !== "object" || !("openapi" in spec) || !("paths" in spec))
        throw new ApiError(502);
      return spec;
    },
  });
  return (
    <Page title="API docs" actions={<Link href={url} target="_blank" rel="noopener noreferrer">OpenAPI JSON</Link>}>
      <Tabs value={selected} onChange={(_, value: string) => setSelected(value)} variant="scrollable" scrollButtons="auto" aria-label="API surface">
        {surfaces.map(surface => <Tab key={surface.id} value={surface.id} label={surface.label} />)}
      </Tabs>
      <QueryState pending={query.isPending} error={query.error} retry={() => void query.refetch()} />
      {query.data && <SwaggerDocument key={selected} spec={query.data} />}
    </Page>
  );
}

function SwaggerDocument({ spec }: { spec: object }) {
  const [host, setHost] = useState<ShadowRoot | null>(null);
  const theme = useTheme();
  return (
    <Paper variant="outlined" sx={{ minWidth: 0, overflow: "auto", bgcolor: "#fff", color: "#111", colorScheme: "light" }}>
      <Box ref={(element: HTMLDivElement | null) => {
        if (element && !element.shadowRoot) setHost(element.attachShadow({ mode: "open" }));
      }}>
        {host && createPortal(<>
          <style nonce={cspNonce}>{swaggerCss + `
            .swagger-ui, .swagger-ui.swagger-ui :not(code):not(pre):not(.microlight) { font-family: ${theme.typography.fontFamily}; }
            .swagger-ui.swagger-ui code, .swagger-ui.swagger-ui code *, .swagger-ui.swagger-ui pre, .swagger-ui.swagger-ui pre * { font-family: ui-monospace, SFMono-Regular, Consolas, monospace; }
          `}</style>
          <SwaggerUI spec={spec} supportedSubmitMethods={[]} plugins={readOnlyPlugins}
            queryConfigEnabled={false} persistAuthorization={false} withCredentials={false}
            docExpansion="list" defaultModelsExpandDepth={-1} deepLinking={false} />
        </>, host)}
      </Box>
    </Paper>
  );
}
