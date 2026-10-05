# Official MUI Template Integration

Source: [MUI templates at 27565bf](https://github.com/mui/material-ui/tree/27565bf06476f6ce2f8eb0c67393c67477968fa2/docs/data/material/getting-started/templates).
MIT notice is included in `public/MUI-TEMPLATE-LICENSE.txt` and built assets.

| Local directory | Upstream template | Responsibility |
| --- | --- | --- |
| `shared-theme/` | `shared-theme/` | One theme for authentication, shell, pages, forms, and dialogs |
| `dashboard/` | `dashboard/` | Permanent desktop navigation, mobile drawer, breadcrumbs, account identity |
| `dashboard/theme/customizations/` | Dashboard chart and data-grid styles | Community MUI X components |
| `sign-in/` | `sign-in/` | Authentication layout and responsive form surface |
| `../console/` | Grove integration | Domain views, API calls, authorization, mutation safety |

## Integration Boundaries

- Preserve upstream typography sizes, spacing, surfaces, and component states.
- Use system fonts and the Grove green brand scale. Typography letter spacing is zero.
- Keep one Emotion cache with the server CSP nonce and one application theme.
- Leave `disableTransitionOnChange` off: MUI's temporary transition style has no CSP nonce.
- Use the documented MUI X CSS slot names in the theme so Grid/Charts remain lazy-loaded.
- `Field` applies the official sign-in form's external `FormLabel` pattern to domain fields.
- Allow multiline fields to grow; retain MUI error, warning, and success semantics.
- Menus use unique ARIA IDs. The account row opens directly with `ListItemButton`; sign-out uses a separate `IconButton` without duplicate account/security shortcuts.
- Disabled buttons clear the template gradient and shadow; contained buttons use theme disabled backgrounds and readable secondary text.
- Product navigation and real API data replace demo content. Community Grid and Charts are used without Pro/Premium dependencies.
- Overview connection geometry remains product-specific. Configuration routes are not live traffic or connectivity checks.

## Verification

Run `npm run build`, `npm run lint`, and Playwright contract/layout tests.
Compare rendered desktop/mobile and light/dark screenshots, including dialogs.
Run `scripts/e2e-console.py --auth-only` from the repository root for the real
Rust API, PostgreSQL, password setup, session invalidation, and permission flow.
