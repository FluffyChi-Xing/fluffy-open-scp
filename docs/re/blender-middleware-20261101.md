# Blender middleware boundary (2026-11-01)

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

