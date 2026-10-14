---
title: GlassBox：how the city runs
description: Subsystem registration, three-stage updates, and simulation versus rendering.
research: true
---
# GlassBox: how the city runs

> Architecture notes · Documentation summary · Compiled on 2026-10-15. This article summarizes archived OpenSCP research; reverse-engineering validation was not repeated during website development.

A visible SimCity (2013) city is not a single rendering object. Terrain, roads, zoning, disasters, audio and simulation systems have separate responsibilities connected by runtime management structures.

## Reading the layers

The archived analysis associates `GB` with engine runtime, `SC` with simulation and related rendering constants, and `SP` with platform and rendering support. Namespaces are organizational clues, not necessarily separate DLL boundaries.

| Layer | Responsibilities identified in the research |
| --- | --- |
| GB | Game management, Eco fields, state machines, synchronization and IO |
| SC | Agents, traffic, Zoning, Lots, Terrain and Disasters |
| SP | Platform support, rendering and shader constant-buffer hosts |

## Finding a subsystem

The research records a GameManager service-locator interface that retrieves subsystems by identifiers. Entries include GraphicsGame, RoadManager, TerrainGame and ZoningGame.

Investigating a visual effect therefore requires more than following an asset file: identify the runtime consumer and the stage at which it updates.

## Three-stage updates

The documented update phases are **Begin → Process → End**. They provide a timing framework, but phase names alone cannot establish all threading relationships or the complete scheduling order.

The E (Eco, simulation) and G (Graphics) sides connect through shared data and descriptors. In the archived terrain example, typed maps are held by reference-counted objects rather than being fully copied every frame.

## What this means for modding

An existing resource is not necessarily consumed in the current game state. Record resource references, subsystem states and in-game observations alongside edits. A preview change alone does not establish a simulation change.

## Evidence and limits

- Original research: `docs/blog/glassbox-engine-architecture.md`.
- Original evidence: decompilation records, real-package probes and runtime observations.
- This summary extracts architectural concepts. Addresses, layouts and implementation details require validation for each game version.
- [Read the archived original, in Chinese](https://github.com/FluffyChi-Xing/fluffy-open-scp/blob/670503b4bdc09c6823ff1bfa32c9de9918167284/docs/blog/glassbox-engine-architecture.md).

Read next: [Decal rendering](./decals) · [Asset formats](./formats)
