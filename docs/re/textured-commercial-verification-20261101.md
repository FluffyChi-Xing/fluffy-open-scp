# Textured commercial in-game verification (2026-11-01)

## Goal

Build one commercial park package whose facade textures are complete, then
compare it in game against the source asset. No placeholder/albedo bake.

## Source

- Geometry: `02_工程_科潘/Copan_OriginalAsset.blend` (8,488 vertices, 4,547 faces).
- Six-slot material: `B4_Lit` + `B4_SixSlots`; slot0..slot5 images loaded from
  the original Copan probe export.
- Native export: `assign_region(..., 'WALL', ...)` + SC5 native exporter.
  This is needed because the plug-in rejects raw original B4 meshes unless a
  native layer exists.

## Output

```text
LOTM: tmp/native-copan-textured/asset.lotm
mesh: 1
material: 1
params: [0.02734375, 0.02734375, 0.0, 0.0, ...]  (native six-slot table)
package: 000_OpenSCP_CommercialTextured.package
size: 9,727,362 B
sha256: 256AF0F9D44752C8DC576C07358024C1CE509787569983A4FA2EE30DFB3AA318
```

Package contents (17 TGIs):

- Dalian identity clone: Menu2 880 B, Unit 3,085 B, Lot 1,395 B;
- Locale table `284C6727`;
- 4 LOD model resources;
- 6 material resources: slot0 params + slot1..slot5 rasters;
- new LOD/mask keys.

## Boundary

The native LOTM is the texture authority. OpenSCP only copies it into the RW4
container and keeps the Dalian menu/unit/lot identity. There is no Rust-side
palette/atlas composition in this build.
