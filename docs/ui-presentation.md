# UI presentation and charts

This pass keeps `src/styles/tokens.css` and business data flows unchanged.

- `src/styles/surfaces.css` owns the explicit card surface list. Cards use existing surface, foreground, border and shadow tokens; blur is progressive enhancement with an opaque fallback. Do not use wildcard selectors that also affect canvas pixels or editor controls.
- `src/styles/flow-presentation.css` styles both workflow and map-generation nodes. Keep node transforms and header heights unchanged: Vue Flow measures handle geometry. Map nodes' `.source` label styling must not inset `.vue-flow__handle-right`.
- `index.html` and `src/styles/startup.css` show the real logo before the application module arrives. The early bilingual markup follows the document language, while `src/main.ts` dismisses it only after router readiness and a Vue render tick. There is no artificial minimum loading time. Native Tauri window creation before HTML is outside this layer.
- Sheet and toast transitions use opacity/transform, with existing global reduced-motion handling. Hidden sheets are inert; slot contents stay mounted.
- Overview data loading uses skeletons. Existing data fetching, stores, IPC, node configuration and graph evaluation are not changed.

## Chart policy

Use **lieflat-charts** for every statistics visualization. Use SVG/CSS implementations, not ECharts. The former ECharts showcase and its unused adapters have been removed; package metadata and lockfile no longer reference ECharts or zrender. Palette roles derive from the project's existing tokens, not new global design tokens.

### Template selection

| Surface | Chosen template and source | Alternatives considered |
| --- | --- | --- |
| Extension count | F4 Tick Donut, `templates/basics-gallery.html`, “Where the traffic comes from”, B4 rendering block | L14 Hundred Field is readable for few categories but duplicates the dial's proportion information; F5 is reserved for capacity so both panels have distinct silhouettes. The dial groups beyond the top five when more than six categories exist; the full category list is preserved. One radial mark is approximately one percentage point. |
| Extension capacity | F5 Tick Rows, same gallery, “Six teams, shipped and counted”, C1 block | L2 Dot Cascade cannot count byte-sized units at this density; F1 vertical Rung Bars cannot fit long extension names. Horizontal tick rows retain a shared zero-based 0–100% domain. CSS renders the gallery's tick queue with exact share lengths; there is no minimum-width inflation for zero/tiny values. All categories remain scrollable. |
| Showcase | F2 Hairline Line, same gallery, “Thirty days of sign-ups”, B2 block | L3 Barcode Lollipop targets much longer series; F3 Hairline Area overemphasizes filled area for only seven days. F2 retains the calendar floor, fine line, daily dots and hollow weekend markers. Solid/dashed lines and exact textual readings distinguish the two series without relying only on color. |
| Existing timing and project statistics | F12 Dumbbell Queue and existing Lieflat SVG/CSS project charts | Retain their data contracts and handlers; apply only shared card surfaces. |

## Validation

- Vue type check, targeted ESLint and Vite production build.
- Sheet open/close/slot regression tests; rendering telemetry regression tests.
- Chart tests check zero values, single-category coverage, exact values with grouped visual summaries, reactive geometry and locale updates.
- Browser inspection: overview at desktop and 390px, both themes, locale switching, sheet open/closed, and both node families in an isolated fixture. Fixtures/build output are under ignored `tmp/`; no real project data is written.

The worktree is shared with other active tasks. Do not stage, reset, or format their files wholesale. The only locale-index edits in this pass rename chart navigation/showcase descriptions; preserve all concurrent messages.
