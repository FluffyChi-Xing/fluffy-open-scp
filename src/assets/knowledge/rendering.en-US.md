# Rendering Pipeline — A Primer

SimCity (2013) runs a D3D9-era deferred pipeline with a data-driven,
multi-sampler material system for building facades. This primer covers the
parts that matter for texturing.

## The six building samplers

The `building4` shader family samples up to six textures per pixel, mapping
1:1 to the six RW4 material slots:

1. **slot0 params table**: rows of float4 — palette UVs, regionXform (the
   Base/Top UV transforms), tilePadding, room size;
2. **slot1 color control map**: RGB = tint region selection, B×2 is the
   brightness multiplier, A = cutout mask (downward-face culling);
3. **slot2 normal map**: standard tangent space, alpha = AO;
4. **slot3 shader map**: G/B = specularity (wall/glass), A = window opening
   position (fake interiors);
5. **slot4 palette**: 512×16, tint lookup colorization + surface reflectance;
6. **slot5 interior map**: prerendered room atlas, alpha = per-window lights.

## Two facade layers (Base / Top)

- **Base layer**: the main wall, UV = facade world projection ×regionXform;
- **Top layer**: window motifs, UV from a second projection (uv2)
  ×regionXform2, blended by slot1's A channel.
- Specularity channels: walls use G, window glass uses B, complementary to
  the window-opening mask A.

## Fake interiors

`ClipAndReliefMapPS` uses the shader map's A channel to find openings,
picks a room per window pane with noise, and box-projects the slot5 room
atlas into the window; at night the alpha channel lights the "lamps".

## Ground and lots

- The LotMask raster's four channels are boolean zones (threshold 128);
- Each channel is tinted by `LotColor.RGB` and `.A` (0–15) indexes into a
  16-tile ground texture atlas;
- Placement comes from properties (inverse `LotPlacementTransform`), not
  from shaders.

> Full reverse-engineering notes live in `docs/rendering.md` and
> `docs/overview/rendering-pipeline.md`.
