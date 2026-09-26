import { RefObject, useEffect, useState } from "react";

type Path = { key: string; d: string; selected: boolean };

export function ConnectionPaths({
  container,
  revision,
}: {
  container: RefObject<HTMLElement>;
  revision: string;
}) {
  const [paths, setPaths] = useState<Path[]>([]);
  // The parent host ref is attached after child layout effects on first mount.
  useEffect(() => {
    const root = container.current;
    const hub = root?.querySelector<HTMLElement>(".hub-label");
    if (!root || !hub) return;
    let frame = 0;
    const nodes = [...root.querySelectorAll<HTMLElement>("[data-connection]")];
    function measure() {
      if (!root || !hub) return;
      const bounds = root.getBoundingClientRect();
      const center = hub.getBoundingClientRect();
      const stacked =
        getComputedStyle(root).gridTemplateColumns.split(" ").length === 1;
      setPaths(
        nodes.map((node) => {
          const rect = node.getBoundingClientRect();
          const left = node.dataset.side === "client";
          const x1 = (left ? rect.right : rect.left) - bounds.left;
          const y1 = rect.top + rect.height / 2 - bounds.top;
          const x2 = (left ? center.left : center.right) - bounds.left;
          const y2 = center.top + center.height / 2 - bounds.top;
          const mid = (x1 + x2) / 2;
          // Stacked layouts route each branch through the outer gutter.
          const start = (left ? rect.left : rect.right) - bounds.left;
          const end = (left ? center.left : center.right) - bounds.left;
          const lane = left ? 2 : bounds.width - 2;
          return {
            key: node.dataset.connection!,
            selected: node.dataset.selected === "true",
            d: stacked
              ? `M ${start} ${y1} C ${lane} ${y1}, ${lane} ${y2}, ${end} ${y2}`
              : `M ${x1} ${y1} C ${mid} ${y1}, ${mid} ${y2}, ${x2} ${y2}`,
          };
        }),
      );
    }
    function schedule() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    }
    const observer = new ResizeObserver(schedule);
    [root, hub, ...nodes].forEach((node) => observer.observe(node));
    schedule();
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [container, revision]);
  return (
    <svg className="connection-paths" aria-hidden="true">
      {paths.map((path) => (
        <path
          key={path.key}
          d={path.d}
          className={path.selected ? "selected" : ""}
        />
      ))}
    </svg>
  );
}
