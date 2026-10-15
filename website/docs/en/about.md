---
title: About OpenSCP
description: An open-source asset browser and modding toolchain for SimCity (2013).
---
# A window into the other side of the city.

OpenSCP is an open-source **SimCity (2013)** `.package` asset browser and modding toolchain. Starting as a modern rewrite of SimCityPak, it connects asset parsing, visual editing, mod building and engine research.

Use it to inspect game resources, preview models and materials, edit properties and text, and export assets to other creative tools.

## Why OpenSCP exists

A building's appearance is spread across models, textures, property lists, decals and runtime rules. OpenSCP brings those clues into a single workbench so their relationships can be read, checked and reused.

The project uses **Rust + Tauri 2 + Vue 3**. Parsers are independent Rust crates with no dependency on the desktop interface. The workbench reads game files you provide on your own machine.

## Three connected directions

| Direction | Focus |
| --- | --- |
| Reliable format foundations | DBPF containers, RefPack decompression, RW4 models and textures, Property lists |
| A practical asset workbench | Resource trees, TGI filters, model and map previews, text and Raster editing |
| Reusable mod workflows | Overlay exports, override detection and planned declarative builds |

Implemented features and future goals are separated on the [development page](./development). Screenshots show the application in development; actual behavior depends on the installed version.

## Research alongside tools

The [research archive](./research/) explains asset formats and rendering mechanisms. Each article should state its evidence, validation method and limits so mod authors and researchers can check the conclusions.

The first website articles are bilingual summaries of existing project documentation, not a complete account of every engine behavior.

## Open source and credits

OpenSCP is MIT licensed. Thanks to the original authors of [SimCityPak](https://github.com/altinctrl/SimCityPak) and the contributors to vgmstream, FFmpeg and other open-source tools.

This is an independent community project, unaffiliated with EA / Maxis. SimCity and related game assets belong to their respective rights holders.

[Get started](./guide/getting-started) · [View source](https://github.com/FluffyChi-Xing/fluffy-open-scp)
