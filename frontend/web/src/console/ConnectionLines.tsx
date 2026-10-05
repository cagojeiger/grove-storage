import { type RefObject, useEffect, useState } from "react";
import { Box, useMediaQuery } from "@mui/material";

export function ConnectionLines({
  root,
  revision,
}: {
  root: RefObject<HTMLDivElement>;
  revision: string;
}) {
  const [paths, setPaths] = useState<{ d: string; selected: boolean }[]>([]);
  const reducedMotion = useMediaQuery("(prefers-reduced-motion: reduce)");
  useEffect(() => {
    const element = root.current;
    const hub = element?.querySelector<HTMLElement>("[data-hub]");
    if (!element || !hub) return;
    const nodes = [...element.querySelectorAll<HTMLElement>("[data-side]")];
    const measure = () => {
      const bounds = element.getBoundingClientRect();
      const center = hub.getBoundingClientRect();
      setPaths(
        nodes.map((node) => {
          const rect = node.getBoundingClientRect();
          const left = node.dataset.side === "client";
          const x1 = (left ? rect.right : center.right) - bounds.left;
          const x2 = (left ? center.left : rect.left) - bounds.left;
          const y1 =
            (left
              ? rect.top + rect.height / 2
              : center.top + center.height / 2) - bounds.top;
          const y2 =
            (left
              ? center.top + center.height / 2
              : rect.top + rect.height / 2) - bounds.top;
          const mid = (x1 + x2) / 2;
          return {
            d: `M ${x1} ${y1} C ${mid} ${y1}, ${mid} ${y2}, ${x2} ${y2}`,
            selected: node.dataset.selected === "true",
          };
        }),
      );
    };
    const observer = new ResizeObserver(measure);
    for (const node of [element, hub, ...nodes]) observer.observe(node);
    return () => observer.disconnect();
  }, [root, revision]);
  return (
    <Box
      component="svg"
      className="connection-paths"
      aria-hidden="true"
      sx={{
        position: "absolute",
        width: "100%",
        height: "100%",
        pointerEvents: "none",
        color: "primary.main",
        display: { xs: "none", md: "block" },
      }}
    >
      {paths.map((path, index) => (
        <g key={index}>
          <path
            d={path.d}
            className={path.selected ? "selected" : undefined}
            fill="none"
            stroke="currentColor"
            strokeOpacity={path.selected ? "1" : "0.4"}
            strokeWidth="1.5"
          />
          <path
            d={path.d}
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeDasharray="4 16"
          >
            {!reducedMotion && (
              <animate
                attributeName="stroke-dashoffset"
                from="20"
                to="0"
                dur="2s"
                repeatCount="indefinite"
              />
            )}
          </path>
        </g>
      ))}
    </Box>
  );
}
