# Native empty-map road preview

Status: 2026-10-08, implemented locally; user visual acceptance and commits pending.

## Desktop integration (2026-10-09)

The user approved the isolated preview and requested a second visual review in
OpenSCP before any commit. The production map command now discovers a matching
region under the OS Documents/SimCity/Games/account/region hierarchy when no
explicit reference is selected. It checks the EGB environment against the selected
region and tries the newest matching references first. Explicit references still
surface validation errors. Missing references leave native roads unavailable;
the previous raster-inferred road mesh is no longer displayed as a substitute.

The production map panel uses `MapResourceControls.vue` in normal and fullscreen
views, with a shared resource selection and opacity. The sidebar contains only
Plots, Vegetation, Water and Road preview. Region/package switches invalidate
pending requests and clear references, preventing stale maps from replacing the
new selection. Both locales explain reference availability and resource data.

Desktop build: `npm run tauri -- build --debug --no-bundle`; executable and registry
resource are in `target/debug`. The older running `D:/openscp` executable requires
a separate restart/update to use this build. Browser checks mounted the actual
production panel with fixture-backed IPC, verified four physical controls, coal
selection, clearing, fullscreen and both locales without runtime errors. The
Rust discovery example separately resolved the user's real Titan reference.
Evidence: ignored `tmp/map-panel-check.json` and `tmp/map-panel-integrated-*.png`.

The preview uses only regional (`0`) EcoPathSet curves. City saves contain copies
of that network plus played roads; those curves must never replace regional roads
inside city bounds. Saved height, forest and soil do not replace original terrain.
City mineral/water maps remain reference snapshots, potentially modified by play,
not evidence of pristine deposits.

`region_state.rs` reads retail v17/v20 states with structural bounds, map sum and
slot validation. Regional units retain original transforms. Titan's local fixture
has 270 regional curve records, zero city curve records and 12 native highway
interchanges. State files and exported game assets stay in ignored `tmp/`.

## Recovered rendering rules

- `SC/cMeshExtrusionComponent/GetSweepInfo.c`: zero interval covers the available
  path; negative length anchors an end segment; rounding rescales offsets, span
  and steps together.
- `anonymous/InitExtrusionComponents.c`: unresolved optional configs fall back
  to the model column. Model properties can refer to another RW4 through
  `00F9EFBB`. Skipping these entries removed bridge decks and cables.
- `anonymous/GenerateExtrusionModel.c`: normalize vertex Y across each sweep step;
  repeated V uses distance divided by transformed model height.
- `anonymous/GenerateExtrusionPropInstance.c`: `mScale.x` is a uniform instance
  scale. Applying all three scale columns independently shortened bridge towers.
- `anonymous/GenerateExtrusionRibbon.c`: repeated V is `uvStart.y - distance /
  worldSize.y`. `scale.y` does not multiply the texture period. Railroad components
  use scale Y=21, explaining the previous stretched sleepers.

Suspension bridge `F77CACD0` resolves cable property `8FFF0DC6` to RW4 `E2C6A2EB`
and texture `DA9B43DC`. Its two `3FE81A70` six-vertex components use lengths
`-0.1` / `0.1`: they close bridge ends, rather than repeat below the deck. Towers
and cable spans share rounded intervals. Texture overrides come from the
extrusion table. Geometry packed into `(x,height,y)` corrects winding before the
shared reflected map root so lighting remains consistent.

## Original city-entry interchange

`SimCity_Game.package` contains property `00B1B104:40E1C000:30360939` and model
`2F4E681B:00000000:30360939` (one mesh, 5,610 vertices, three embedded textures).
Property `00F9EFBB` references the mesh; `0CC8FE7A` holds internal road types and
`0CAA680D` contains control points. Regional unit transforms place the actual mesh
without inferring junction shapes. Only units with embedded road definitions
enter this layer; editor markers and played-city buildings are excluded.

## Validation and remaining work

Local browser evidence: `tmp/map-corrections-check.json` and
`tmp/terrain-color-titan/corrected-{bridge,bridge-side,junction,rail}.png`.
Browser checks report 270 regional curves, zero city curves, 12 interchanges,
zero unresolved extrusion components and no runtime errors for this fixture.
The earlier native preview check also exercised coal/ore/oil/water switches.

Near trees use four original RW4 models and distant trees use an atlas rendered
from those models. Placement still uses preview sampling. Exact forest Halton
placement, full normal/specular shading and extrusion intersection clipping
remain follow-ups. Repeating model sweeps currently approximate adaptive
curvature subdivision with 8 m steps.

Extracting the pristine baseline from packages/templates remains necessary for
a standalone map editor. For now a selected local region state supplies reference
paths and units; it is not presented as a reconstruction of a played city.
The future schema must distinguish original map assets from reference snapshots
and retain TGI, unit transforms, spline topology and extrusion definitions.
