---
title: Workbench guide
description: Browse assets, preview models and maps, and export overlay packages.
---
# Explore, edit, then build

OpenSCP panels address different parts of the same asset pipeline. This page explains their uses and their boundaries.

## Asset browser

Open a package and find resources by type or TGI. Text uses syntax highlighting; unknown formats can be inspected in the hex view. Open packages, filters and pagination are retained when navigating between pages.

![Asset browser](/images/en-US/source-parsing.png)

### Resource overview

Review scanned packages, resource counts and type and size distributions before choosing a package to inspect more closely.

![Resource overview and type distributions](/images/en-US/resource-overview.png)

## Models and materials

The RW4 viewport displays models, LODs and material channels. The Property editor shows Lot lights, decals, Props and path units. Time-of-day and power states help compare building appearances.

Preview fidelity is evolving. Do not assume every preview exactly reproduces the original game.

![Model and material preview](/images/en-US/rendering.png)

## Maps and Raster

The map panel places regional terrain, city lots and resource layers in a shared spatial context. Raster painting provides a four-channel LotMask canvas with brushes, fills and parking markings, plus overlay package exports.

![Map preview](/images/en-US/map-review.png)

![Four-channel Raster painting workbench](/images/en-US/raster-paint.png)

## Mod Studio

| Panel | Usage | Status boundary |
| --- | --- | --- |
| TGI override detection | Scan game and mod directories for override chains and differences | Listed as available in the README |
| Locale editor | Filter by stringID, edit text, revert rows and export an overlay | Listed as available in the README |
| UI preview | Inspect game UI assembled from CSS, locale text and sampled images | Preview; full editing is still evolving |

## A suggested validation workflow

1. Select a resource and record its TGI and original values.
2. Make one change that can be compared with the original.
3. Export an overlay and check the written resources.
4. Use override detection to inspect conflicts with other mods.
5. Validate in the game and keep the result and a way to revert.

Source: [Project README](https://github.com/FluffyChi-Xing/fluffy-open-scp#readme). Screenshots match the selected website language.
