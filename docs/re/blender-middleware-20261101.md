# Blender middleware boundary (2026-11-01)

> **2026-11-06 已被取代。** 本文的“GLB 归一化回退”不再是 `.blend` 导入路径。
> `.blend` 导入现在只允许 SC5 Asset Editor 原生导出并只保存 LOTM；
> 见 `docs/re/blender-native-export-migration-20261106.md`。

## Decision

OpenSCP does not reimplement model or facade-texture composition. Blender is
invoked as an external preparation service; the workbench owns only scene
orchestration (path, prop, effect, spawner, lot/menu/unit).

The import contract is:

```text
.blend -> Blender background service -> project GLB + six-slot images
```

No Blender add-on installation is required for this path: the script reads the
node groups, images and UV layers already stored in the `.blend`.

## Why not a generic bake

Two prototypes were checked before choosing the boundary:

1. **Generic PBR bake** (`Base Color` -> emission, normal bake): works for
   ordinary Principled materials, but B4 six-slot materials do not put their
   final colour in `Base Color`. The bake produced a black atlas. The final
   colour is `palette[slot1.rg] * slot1.b`, which needs the native recipe.
2. **Plugin native export** (`native_asset._native_export_impl`): correct for
   assets authored by the plug-in, but explicitly rejects original B4 meshes
   (`请选择插件制作的楼体；原工程 B4 网格请使用编辑器原有导入路径`).

Therefore a built-in generic baker would be worse than the existing six-slot
extraction. The safe default is normalisation, not a fake bake.

## Implemented seam

`src-tauri/src/blender_service.rs` owns:

- Blender executable discovery (`BLENDER_PATH`, `D:\blender`, Program Files);
- Windows extended-path conversion;
- background process launch with captured diagnostics.

`mod_flow_assets::import_blend` now:

- checks a content-addressed cache (`.openscp-blend-cache/<hash>/result.json`)
  before starting Blender;
- tries the optional native adapter when `OPENSCP_BLENDER_ADDON` or a bundled
  `resources/blender/ny_facade` is present;
- falls back to normalisation when the native adapter rejects original B4
  meshes (the rejection is explicit, never silently baked);
- excludes Lot/ground objects (`ny_lot_regions`);
- selects renderable building meshes;
- joins them into one object and applies transforms;
- keeps the existing six-slot image/params extraction;
- exports one preview GLB.

The import result now carries `nativeAdapter.status` (`native`,
`unsupported-b4`, `unavailable`, `failed`) and `cache` on a hit. A native
adapter success also stores the `.lotm` next to the preview GLB and marks the
workbench session `nativeBlender`, so the six-slot binding tab is hidden:
Blender owns those textures. The native adapter's `.lotm` path is persisted as
`nativeLotm` in the workbench source block, ready for the package builder.

This makes the workbench consume one clean building while preserving the
source material binding. It does not invent texture colours.

## Future native path

When a plug-in-authored asset is detected, a second adapter can call the
bundled plug-in's native export and store the resulting GLB/LOTM. This is an
optional capability, not the default for original B4 meshes. The back end must
not silently fall back from native export to a PBR bake.

The exporter is staged into `src-tauri/resources/blender/ny_facade` by
`scripts/stage-blender-addon.ps1` and included in the Tauri bundle resources.
At build/package time this is a bundled dependency; at development time
`OPENSCP_BLENDER_ADDON` points at the same package. The staging script is
explicit because the exporter and atlas library are third-party assets.


## Throughput: where external .blend import time actually goes (2026-11-05)

Profiled `entity/tall_office.blend` (2145 faces, 276 six-slot materials) with
Blender 5.1 headless, timing each stage of the embedded import script:

| stage | before | after |
| --- | --- | --- |
| `open_mainfile` | 1.75 s | 1.65 s |
| join + `transform_apply` | 0.00 s | 0.00 s |
| material classification | 0.08 s | 0.06 s |
| UV domain normalization | 0.01 s | 0.01 s |
| **glTF export** | **1336.6 s** | **0.98 s** |
| slot extraction | 10.6 s | 0.68 s |
| **total** | **~22.5 min** | **~4.9 s** |

Mesh processing was never the problem. The glTF exporter walked every
six-slot node group per material (~4.8 s each, one
`More than one shader node tex image` warning per material). Slot extraction
re-rendered the same atlas once per material: 1385 PNGs for 21 unique images.

Fix, in the embedded script:

1. Capture each material's slot binding (image datablocks) before touching
   any node tree, then replace every six-slot graph with a minimal
   Principled + tint texture. Material names and mesh/material indices are
   untouched, so `materialSlotsByMaterial` binding and TEXCOORD_0..7 survive;
   the GLB is only the coarse preview, the extracted six-slot payload stays
   authoritative. GLB embedded images drop 85 → 4, size 2.2 MB → 1.0 MB.
2. Render each unique atlas image once and share the file path across every
   material that references it (31 files instead of 1385).

Equivalence checked byte-for-byte against the pre-change output: 276/276
materials, 0 slot-set differences, 1380/1380 slot bindings byte-identical,
identical `material_params.json`, identical GLB material names and attribute
sets.

### Why the plug-in is fast (SC5 Asset Editor 1.23.5)

`ny_facade/native_asset.py` never calls `bpy.ops.export_scene.gltf`. It
assembles the GLB itself (`GLB` class: numpy arrays → bufferViews/accessors,
SHA-256 image dedupe), reads atlases once per `(pack, slot)` into numpy via
`pixels.foreach_get`, and dedupes materials by a recipe key
(`groups[json.dumps([kind, entry, palette_row, color, ...])]`). `channels.face_state`
caches per material index, and `material_state.recover` is only run once per
material index. Its LOTM payload is written directly (LOTM v9 header, per-group
GLB chunks + five texture maps + params), so no Blender exporter is involved
at any point.

The normalization path cannot reuse the plug-in's LOTM writer (it rejects
original B4 meshes by design), but the two throughput techniques that matter
for it — flatten before export, one atlas write per unique image — are now in
place.
