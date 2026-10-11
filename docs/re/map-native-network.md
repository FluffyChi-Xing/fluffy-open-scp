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

## 道路外观 Property 的字段级含义

本轮对 `00B1B104:09558B7E:DC6C325F` 和路径条目
`00B1B104:40E1C400:017A430A` 做了完整转储。截图中看到的
`0x09558829`～`0x0955882D` 不是区域道路中心线坐标，它们是挤出组件表的
逐行参数；`0x0955882C` 才是每个组件的三维局部偏移。`DC6C325F` 有 28 行，
而 `017A430A` 的静态挤出配置有 12 行，行号与组件 Key 数组
`0x09558827` / `0xBCC53802` 对齐。

| 属性 | 当前引擎用途 | 证据 |
|---|---|---|
| `0x09558821` | MeshExtrusionMaterial 的材质 Key | `cMeshExtrusionMaterial::Init`，现有 `road_assets` 已解析 |
| `0x09558827` | 每行的模型/组件回退 Key | `InitExtrusionComponents`；组件 Key 无法解析时回退到该列 |
| `0x09558828` | sweep 起始距离/起始偏移 | `RoadSweep.start` |
| `0x09558829` | 重复间隔（interval） | `RoadSweep.interval` |
| `0x0955882A` | sweep 长度；负值表示从末端锚定 | `GetSweepInfo` 的负 length 分支 |
| `0x0955882B` | 重复步长（step） | `RoadSweep.step` |
| `0x0955882C` | 组件局部 XYZ 偏移 | `GenerateExtrusion*` 前的父级 offset 累加 |
| `0x0955882D` | 是否沿曲线扭曲/跟随切线 | `RoadSweep.distort` |
| `0x0955882E` | 是否重复 UV | `RoadSweep.repeat_uv` |
| `0x0A49AEA4` | 每行纹理覆盖 Key | 组件纹理不存在时的覆盖链 |

`DC6C325F` 的 `0x0955882C` 包含 `[24,0,0.1]`、`[-24,0,0.1]`、
`[20.5,0,-23.5]` 等成对值，说明它描述的是道路截面上的车道、护栏、
桥梁或附件相对位置；它不提供道路在 CapeTrinity 世界中的平移。世界位置
仍来自 `30360939` 的 UnitTransform，路径曲线则来自 `0x0CAA680D` 等字段。

路径条目 `017A430A` 同时包含：

```text
0x09532375 → static extrusion 24789770
0x09532377 → interactive/realtime extrusion 2363035178
```

因此真实道路渲染链已经可以完整表达为：

```text
EGB/UnitTransform + EcoPathSet 曲线
 → PathEntry (40E1C400)
 → static/realtime extrusion (09558B7E)
 → component/model/texture/material
```

当前 OpenSCP 缺少的是区域初始 `UnitTransform` 和运行时切分后的
EcoPathSet；道路外观 Property 本身已经足够支持真实材质、车道宽度、桥梁
附件和重复模型的还原。
