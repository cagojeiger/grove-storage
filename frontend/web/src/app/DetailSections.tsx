import { useId, type ReactNode } from "react";
import { Box, Stack, Tab, Tabs } from "@mui/material";
import { useRoute } from "./navigation";

export function DetailSections({
  label,
  sections,
}: {
  label: string;
  sections: { value: string; label: string; content: ReactNode }[];
}) {
  const id = useId();
  const route = useRoute();
  const [path, query] = route.split("?");
  const params = new URLSearchParams(query);
  const selected =
    sections.find((section) => section.value === params.get("tab")) ??
    sections[0];
  if (sections.length === 1) return selected.content;
  return (
    <Stack spacing={3}>
      <Tabs
        aria-label={label}
        value={selected.value}
        variant="scrollable"
        scrollButtons="auto"
        sx={{ borderBottom: 1, borderColor: "divider" }}
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
      >
        {selected.content}
      </Box>
    </Stack>
  );
}
