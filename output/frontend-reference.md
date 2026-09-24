# FileGate frontend reference

Reference reviewed: NoteGate `origin/main` at `419d3ddf` (2026-09-14 release 0.1.83). The local NoteGate checkout has diverged, so source inspection used `git show origin/main:...` without changing that checkout.

## Adopted In The Preview

| NoteGate pattern | FileGate application |
|---|---|
| Semantic color tokens | Background, surface, text, muted, selection, border, danger and focus colors have light/dark values |
| System UI font with Korean fallbacks | Same font-stack approach; no external font dependency |
| 28px desktop controls, 44px mobile controls | Compact desktop toolbar and touch-sized mobile actions |
| 767px mobile boundary | Mobile list layout below 768px; sidebar/table from 768px |
| 4px control and 6px surface radii | Restrained input/button treatment |
| Active edge plus selected surface | Current navigation is visible beyond text color |
| Primary/secondary/ghost/danger variants | Deletion is visually distinct from ordinary primary actions |
| Shared modal behavior | Escape, focus containment and focus restoration |
| Reduced motion | Nonessential transitions disabled when requested |

## Implementation Direction

The current deliverable is an interactive mockup with sample data. React integration, authentication and server calls are not implemented.

Use NoteGate's React + TypeScript + Vite foundation, semantic CSS tokens and shared UI ownership as the reference when building the actual frontend. Tailwind may supply layout utilities, with feature components consuming semantic tokens. Use TanStack Query for server data if adopting the same stack; introduce additional client state tooling only where needed.

```text
frontend/web/src/
  app/                 composition and routes
  api/                 transport and response types
  auth/                single-admin session
  design/              theme tokens
  shared/ui/           Button, IconButton, Field, Tabs, Modal
  layout/              console shell and navigation
  features/
    overview/
    storages/
    clients/
```

Keep FileGate's three-section navigation, current neutral/rose palette and single-admin login proposal. NoteGate's editor groups, file tree, Google OAuth flow and document-preview dependencies are outside this console's current requirements.

For the actual application, theme selection should persist in browser preferences and system mode should observe OS appearance changes. The inline mockup uses host-provided appearance and widget state.

## Source Pointers

- [Package and toolchain](https://github.com/cagojeiger/notegate/blob/419d3ddf/frontend/web/package.json)
- [Theme tokens](https://github.com/cagojeiger/notegate/blob/419d3ddf/frontend/web/src/design/theme.css)
- [Button variants](https://github.com/cagojeiger/notegate/blob/419d3ddf/frontend/web/src/shared/ui/Button.tsx)
- [Modal behavior](https://github.com/cagojeiger/notegate/blob/419d3ddf/frontend/web/src/shared/ui/Modal.tsx)
- [Mobile detection](https://github.com/cagojeiger/notegate/blob/419d3ddf/frontend/web/src/shared/hooks/useMediaQuery.ts)
- [Layout policy](https://github.com/cagojeiger/notegate/blob/419d3ddf/frontend/web/src/layout/workbenchLayout.ts)

## Verification Scope

Preview checks cover light/dark appearance, 320/390/767/768/1024/1440px layout, registration, deletion guards and modal keyboard behavior. These checks do not establish production authentication or API compatibility.
