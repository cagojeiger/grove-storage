import { useId, type ReactNode } from "react";
import { Box, Stack, Tab, Tabs, useMediaQuery, useTheme } from "@mui/material";
import { useRoute } from "./navigation";

export function DetailSections({
  label,
  sections,
}: {
  label: string;
  sections: { value: string; label: string; content: ReactNode }[];
}) {
  const id = useId();
  const vertical = useMediaQuery(useTheme().breakpoints.up("lg"));
  const [path, query] = useRoute().split("?");
  const params = new URLSearchParams(query);
  const selected =
    sections.find((section) => section.value === params.get("tab")) ??
    sections[0];
  if (sections.length === 1) return selected.content;
  return (
    <Stack direction={{ xs: "column", lg: "row" }} spacing={3}>
      <Tabs
        aria-label={label}
        value={selected.value}
        orientation={vertical ? "vertical" : "horizontal"}
        variant="scrollable"
        scrollButtons="auto"
        sx={{
          flexShrink: 0,
          width: { lg: 176 },
          borderRight: vertical ? 1 : 0,
          borderBottom: vertical ? 0 : 1,
          borderColor: "divider",
        }}
      >
        {sections.map((section) => {
          const next = new URLSearchParams(params);
          next.set("tab", section.value);
          return (
            <Tab
              key={section.value}
              value={section.value}
              label={section.label}
              id={`${id}-${section.value}`}
              aria-controls={`${id}-panel-${section.value}`}
              component="a"
              href={`#${path}?${next}`}
            />
          );
        })}
      </Tabs>
      <Box
        role="tabpanel"
        id={`${id}-panel-${selected.value}`}
        aria-labelledby={`${id}-${selected.value}`}
        sx={{ flex: 1, minWidth: 0 }}
      >
        {selected.content}
      </Box>
    </Stack>
  );
}
