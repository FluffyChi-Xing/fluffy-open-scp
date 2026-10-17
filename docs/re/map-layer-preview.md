# Map layer preview — 2026-10-08

## Implemented

The existing map panel controls now reach both embedded and fullscreen 3D viewers.
Vegetation, water, road preview and plot outlines have independent switches. Soil,
forest density and groundwater are sampled from the exported ecology field in the
terrain material, using the same cropped world origin, spacing and source texture
resolution as the base terrain. Switching layers changes uniforms without rebuilding
geometry or resetting the camera. The panel provides a relative-intensity legend and
opacity control. Resource mode temporarily hides water, trees and road meshes.

The ecology channel contract remains R=soil, G=forest, B=groundwater. No mineral
deposits are inferred from city locations or random seeds. Coal, oil, ore and
desirability controls are disabled with a localized missing-data explanation because
the selected static terrain package supplies brush references, not their raster data.
The older 2D `synthesize_resource_layers` implementation is deliberately not reused.

`map-vegetation.ts` places deterministic preview trees according to actual G density,
in spatially culled instanced batches. It excludes water, steep slopes and nearby
candidate roads. Geometry and placements are approximations, not the original tree
species or saved tree instances. The existing distant canopy tint remains.

`map-water.ts` clips the water plane against the source height field and uses depth
color, Fresnel reflection, specular waves and shoreline foam. Waves use a generated
periodic normal texture rather than the game's FFT simulation. Visible water updates
at 30 fps; resource mode disables its rendering. No-water regions skip wave redraws.

`map-roads.ts` isolates the existing raster-to-ribbon preview and adds distance-based
UVs and a generated lane texture. Candidates require both forest and groundwater to
be zero and a narrow strip surrounded by nonzero values. This reduces large dry-area
false positives but cannot identify every real road. The guessed bridge piers were
removed. Width, lanes and water crossings remain approximations pending EcoPathSet.

## Source evidence

Paths below are relative to the local development source tree under
`D:/ea-games/simcity_dev/new-cource/SimCity2013-source-tree/src/`.

- `SC/cWaterTextureSet/Init.c`: binds `waterHeight`, `waterNormals`, `waterChoppy`
  and `beach_foam`. The first three are runtime textures, not named static assets
  found in the scanned retail App/Graphics/Game/EP1 packages.
- Retail App resource `2F4E681B:00000000:A8EAF18E` is `beach_foam`: RW4 texture,
  decoded to a 256×256 PNG by the backend. Assets stay in local packages and `tmp/`.
- `SC/cTerrainForest2/GetForestTexture.c`: forest color/normal textures come from
  the forest render targets. `CreateAndRegisterImpostorTypeIDs.c` constructs per-tile,
  per-density-level impostor IDs; no single static tree image substitutes for this.
- `SC/cTerrainEcoMap/Init.c`: resolves a typed CPU map and optional texture key
  `0x0AF3DAC7`; the static exported ecology field contains only the established
  soil/forest/groundwater channels.
- See `map-bounds-roads-and-bridges.md` for EcoPathSet, extrusion and bridge evidence.

## Remaining work

- Locate the relevant saved region/city state or missing brush raster assets and
  decode coal, oil, ore and desirability fields. Brush centres alone are insufficient.
- Decode real EcoPathSet splines, heights, PathEntry/extrusion materials, bridge type
  and attachments. Do not describe the current ribbon preview as original roads.
- Restore forest species/impostor rendering and match game wave simulation tuning.
- User visual review; no commit has been made for this map layer work.

## Follow-up: native water parameters and forest LOD

`region_3d::region_water_params` now reads the selected region's
`00B1B104:<region>:2FFD7EED` (tessendorfWater), following
`SC/cTessendorfWater/FillFromProps.c` (0x4362F0):

| Property | Meaning | Titan / Horizon / Cape retail values |
| --- | --- | --- |
| `5EF6BAD4` | simulation time step factor | 5 |
| `377C2266` | specular exponent | 500 |
| `72BF2867` | specular strength | 11 |

The DTO preserves missing values as null and rejects non-finite/negative parameters.
The frontend retains finite preview defaults for older fixtures. Zero animation speed
and zero specular strength are valid. `map_layer_assets` exports the source parameters
for reproducible local browser fixtures. These settings do not reproduce the FFT.

Recovered HLSL fragment 217 (waterPS) establishes that **foam coverage uses alpha**,
not the red channel used in the first preview. The preview now uses the native foam
RGB, alpha-squared coverage, depth constants 3.5 / 7.75, 2–10 km view-depth fade,
foam specular suppression, and the source Fresnel expression. Foam has mip filtering
and the shoreline uses a screen-footprint fade. Missing foam assets have zero coverage.
World tiling, generated wave normals, sky color and basin/refraction remain approximate;
the source's five-tap height smoothing and runtime wave-height input are not reproduced.

Forest batches now have two geometry LODs: near crowns plus trunks (100 triangles/tree),
and distant crowns (20 triangles/tree) beyond 3500 m with 15% hysteresis. Both levels
share the same instance transforms and colors; world-cell seeds also determine color.
The renderer's ordinary LOD update selects the level without rebuilding vegetation.
This reduces distant **tree geometry** by 80%; it is not an 80% total-frame speed claim.
These are still procedural preview trees, not the original forest impostors.

Further road tracing located `GB/cEcoPathSet/Read.c` (0x635AB0), called by
`GB/cEcoGame/Read.c` after maps and units. It accepts path-set versions 4–6 and reads
big-endian point/tangent slots, live segment records, point metadata, paths and groups.
`GB/ReadSlots.c` and `ReadSlots__2.c` identify free-slot tables (ref slots derive live
stamps from them). This closes part of the binary-format evidence, **not** the binding
from a retail region template to its saved path payload. Do not scan arbitrary bytes
for plausible coordinates and treat them as authoritative road geometry.

Follow-up validation: 12 frontend tests, one Rust parameter parsing test, Vue typecheck,
focused ESLint and Rust application library check pass (existing Rust warnings remain).
Titan, Horizon and Cape browser captures exercise overview/zoom, groundwater view,
clear/rebuild and idle rendering with water disabled; no WebGL/runtime errors and zero
extra idle frames. Local report: `tmp/map-preview-progress-check.json`; captures:
`tmp/<fixture>/progress-{overview,close,groundwater}.png`.

## Follow-up: Titan Gorge mirror correction

The old preview packed game `(x, y, height)` as Three `(x, height, y)`.
That swaps two axes without a sign change and reflects the map. The coordinate
contract is now `(x, height, -y)`, as specified in `map-3d-preview-research.md` §4.2.
`map-coordinates.ts` owns both directions. All layer builders remain map-local
under one `map-world` root with `scale.z = -1`; shaders invert the boundary before
sampling original fields. Do not flip source texture rows or individual layers.
Camera collision, terrain LOD, initial camera, sun and reset use the same convention.

The asymmetric comparison anchor is Titan site 1029, 定居者之地, at
game `(-1728, 5536, -833.9)`. Viewing from game −Y puts the river bay on the
lower-left and restores the game's highway bend direction. Local evidence:
`tmp/terrain-color-titan/orientation-settlers.png`,
`tmp/map-orientation-check.json`, and the fixture URL
`/tmp/map-layers-preview.html?fixture=terrain-color-titan&site=1029`.
Three coordinate tests cover handedness, inverse field registration and camera
orientation. All 15 related frontend tests, Vue typecheck and focused ESLint pass;
Titan/Horizon/Cape WebGL checks report no errors and no idle redraws with water off.
User visual acceptance is still pending; changes remain uncommitted.

## Follow-up: original forest model discovery

Retail App properties follow this chain (type `00B1B104`, group `40002D00`):
`BA378A12` (Impostor_Forest) → parent `71CB50C9` → property `0DDDED9F`
class `C602CD31` → parent `5F804D7E`. Its `0BD62577` array references four
models. `SC/cImpostorRenderer/InitImpostorClass.c` passes those keys to
`SP::CreateModelInstance`; it renders an offscreen atlas, not a single static texture.

The `map_layer_assets` probe now confirms all four RW4 resources in
`SimCity_Graphics.package`, type `2F4E681B`, group zero:

| Instance | Mesh #7 vertices | Embedded texture sizes (#11 / #13 / #15) |
| --- | ---: | --- |
| `4DF43690` | 732 | 256×512 / 128×256 / 128×128 |
| `C2FBD178` | 4654 | 256×512 / 128×256 / 256×512 |
| `1113B131` | 2935 | 256×512 / 128×256 / 256×512 |
| `89D658DF` | 2651 | 256×512 / 128×256 / 256×512 |

Each has one material section. A foliage image extracted from `4DF43690` alone
is therefore not evidence that the resource is texture-only. Texture slot roles,
seasonal materials and alpha handling still need to be decoded before integration.
The class's `0E0B99FD` entries are environment keys, not additional tree models.
The inherited bounding boxes have heights 47 / 44 / 41 / 30.5 respectively;
atlas dimensions are 128×128 and material override is `CE5BBF5E`.

`SC/cTerrainForest2/Init.c` initializes 20,000 samples, base `0xD6F8`, four
types. `SC/cDistributeSampleState/Init.c` uses HaltonSequence3; density/LOD tile
selection and sample updates need further tracing. Current preview placement and
geometry remain procedural; locating the real assets does not reproduce placement.
Reproduce the resource audit with:
`cargo run -p sc-properties --example map_layer_assets -- <game-data>`.

## Validation

- Eight Vitest tests cover crop coordinates, ecology channel mapping, deterministic
  tree seeds, rejection of broad dry patches, and terrain skirts.
- Vue type checking, focused ESLint and Rust application library checking.
- `map_layer_assets` resolves and exports the real foam resource.
- Titan Gorge browser preview captures vegetation/water/roads, each ecology layer
  and all physical layers off without shader/runtime errors. Local fixtures and
  captures are in `tmp/terrain-color-titan`; preview: `tmp/map-layers-preview.html`.
