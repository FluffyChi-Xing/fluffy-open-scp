---
title: Development progress
description: Available capabilities, evolving editors and the planned Modding Suite.
---
# Visible progress. Honest boundaries.

**Content snapshot: 2026-10-15.** This page summarizes the project README and roadmap. It is not a live CI dashboard and does not imply that every feature ships in every release. Check individual release notes.

## Available: workbench and export foundations

- DBPF / RefPack, RW4 and Property parsing; model, texture and text previews.
- TGI override detection across original packages and mods.
- Locale editing and deterministic overlay package exports.
- Roadmap execution records mark the heightmap write-back library, debug-tools overlay command and blueprint foundation spike as completed.

“Available” indicates an implementation or delivery record, not coverage of every format variant.

## In progress: editors and previews

The README identifies the Property editor, game UI editing pipeline, material fidelity, time-of-day and power-state rendering as ongoing work.

Game UI preview and a complete editable UI workflow are different milestones. A visible panel in a screenshot does not imply that all its functions are finished.

## Planned: declarative Modding Suite

The proposed `openscp.mod.toml` project manifest connects:

- A complete workflow for OBJ imports and LOD management;
- Resource dependency resolution and automatic validation;
- Deterministic overlay builds and actionable diagnostics;
- Previews, file watching, and later advanced material and skeleton editing.

These are directions, not promised delivery dates.

## Follow changes

Read [Releases](https://github.com/FluffyChi-Xing/fluffy-open-scp/releases) for shipped changes and [Issues](https://github.com/FluffyChi-Xing/fluffy-open-scp/issues) for public discussions and reported problems.

Sources: [README](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/README.md), [development priorities](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/roadmap/priorities.md), and [Modding Suite roadmap](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/roadmap/modding-suite.md). These follow the current source layout and may move when branches are reorganized.
