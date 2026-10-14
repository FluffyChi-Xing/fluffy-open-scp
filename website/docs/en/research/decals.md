---
title: How a decal reaches a building
description: Property dictionaries, texture containers, projection volumes and material channels.
research: true
---
# How a decal reaches a building

> Rendering notes · Documentation summary · Compiled on 2026-10-15. Based on existing probes, shader analysis and game comparisons; this article adds no new measurements.

Signs, graffiti, holes and ruined wall patches can be studied through the same questions: how are they described, where is the texture, how is it projected, and which material semantics apply?

## A dictionary is not an image

The archived analysis identifies the decal atlas as a **Property resource**, containing material references, texture sizes and entry fields. Entries use parallel arrays: elements at the same index describe one decal.

Finding the atlas and finding its actual texture are therefore separate steps.

## More than one texture container

A texture may be a raw Raster (`0x2F4E681C`) or embedded in RW4 (`0x2F4E681B`). References can cross base-game and expansion packages, so opening one package may not resolve a complete decal.

## From transforms to projection volumes

The archived shader analysis shows positions transformed into texture space, then clipped against a projection box. A key expression recorded in the source is:

```glsl
clip(1 - abs(texturePosition));
```

This illustrates the original shader semantics; it is not a complete runnable GLSL program. UVs derive from normalized coordinates inside the box. Incorrect axes, matrix conventions or size semantics can visibly misalign the result.

## Appearance alone does not establish semantics

Commercial signs, graffiti and damaged walls differ in material and channel handling. Low-resolution textures projected over several meters can naturally appear pixelated; this alone does not prove a decoder bug.

A better validation order is to check entry references, identify the texture container, compare transforms and projection dimensions, and then verify channels and materials.

## Evidence and limits

- Original research: `docs/blog/decal-rendering-pipeline.md`.
- Implementation references: `crates/sc-properties/src/decal.rs`, `crates/rw4/src/raster.rs`, `src/lib/decalProject.ts`.
- The original combines real-package probes, shader excerpts and game comparisons. Sample-specific findings are not universal rules for every decal family.
- [Read the archived original, in Chinese](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/blog/decal-rendering-pipeline.md).
