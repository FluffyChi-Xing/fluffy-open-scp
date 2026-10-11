# Region bootstrap and first EGB write

This note records the engine-side evidence for a new SimCity region. It is
based on the decompiled game sources in `docs/source-code/SimCity-dev-ida.c`
and the local save manager dump in
`docs/source-code/_legacy_ghidra_dump/GB_cLocalSaveManager.c`.

## What exists before the first EGB

`SC::cGameAppMode::CreateGames` creates the active and region `GB::cEcoGame`
objects, calls `Init`, and registers the standard handlers. This is object
construction only; it does not read `state_file_*.egb`.

The first actual game setup is:

```text
cEcoGame::StartGame(gameID, environmentID, addOns)
  -> GetCurrentGameScripts(gameID, addOns)
  -> read SimProperties / NonSimProperties
  -> create cEcoMap objects from script map entries
  -> initialize map layers from an image resource
  -> create handlers and path/zone/unit state
  -> StartGameHandlers()
  -> PrecreateUnits()
  -> PrecreateZones()
```

`StartGame` stores `environmentID` and passes it to `InitGame`. `InitGame`
constructs every `cEcoMap` from the map-entry property list. For each map,
`cEcoMap::Create` first allocates a `cellsX * cellsY` array, then looks up the
local key in property `0x7E0AE06`. When `environmentID` is nonzero it replaces
the key group with `environmentID` and calls `InitFromImage`. The image loader
tries resource type `65282544` (16-bit image) and then `65282540` (8-bit image),
scaling image samples into the map's `mMaxValue` range. This is the source of
terrain/resource/height-style map layers during bootstrap; no EGB is needed.

`cEcoMap::Create` may additionally apply property `0x7E0AE02` random splats,
with radius `0x7E0AE03`, values `0x7E0AE04`, and optional wrap-around
`0xB149F1B` (decompiler decimal id `185900827`). These are generated in
memory from the game RNG and are also independent of EGB.

## Where initial units and zones come from

`PrecreateUnits` reads table properties `0x0ADFE207` (unit keys) and
`0x0ADFE208` (transforms) from `SimProperties`, then repeats the same lookup
for every script bundle. Each row calls `cEcoGame::CreateUnit` with the
property-provided `cSPTransform`.

`PrecreateZones` does the equivalent for cell zones using properties
`0x0B4D6794` and `0x0B4D6795`, and creates path zones from
`0x0D4130F3`. Therefore a city lot or starter building that is present in a
new region must originate in these precreate property tables (or in later
rules), not from a missing EGB.

## Where initial roads come from

`GB::cEcoGame::StartGameHandlers` starts handlers, then iterates the in-memory
path slots and calls each handler's `PathCreate`. The road graphics handler
(`SC::cGraphicsGame`) and road manager consume the same `cEcoPathSet`.

The path property decoder is `SC::CreatePathsFromProps`. During unit creation,
`SC::cRoadManager::ReadUnitPathData` passes the unit's `mNonSimProps` to this
decoder. Thus the starter road network is normally carried by the precreated
unit/template property lists (and any path state loaded from a save), rather
than by a generic terrain image. It reads:

* path ranges: `0x0CAA6841`;
* point positions: `0x0CAA680D`;
* point tangents: `0x0CB00ED8`;
* point indices: `0x0CAA6832`;
* path entry keys: `0x0CC...` table property `214498938`;
* optional scope and road-connect arrays: `228216404` and `228238011`.

For every accepted range it copies the referenced `cSPVector3` points and
calls `cUnitPathCreationLocal::CreatePath`, which calls
`cEcoPathSet::AddPoints` followed by `AddPath`. Tangents and distance arrays
are then attached to the created segments. This is the authoritative source
of continuous starter roads. An ED polyline approximation cannot reproduce
intersection snapping, segment indices, tangents, or road connection flags.

`SC::cRoadManager::StartGame` does not invent the topology. It reads the
already-created `cEcoPathSet`, builds spline distance arrays, and loads path
flags/speeds/interchange properties from `NonSimProperties`. Graphics path
creation then derives intersection and model data from the same segments.

## Why the first EGB can be absent

The local save manager function at `FUN_00C156E0` is a load-existing-state
path. It constructs `state_file_0_<id>.egb`, opens it, reads it, and returns
failure when the stream cannot be opened or validated. It does not explain
new-region creation because the new-region branch has already produced an
in-memory `cEcoGame` through `StartGame`.

The save manager later serializes that live object/delta state through its
virtual save methods and writes the `state_file_*.egb` files plus `misc/MetaData`.
The exact writer is dispatched through the save-manager vtable in the dump, so
the first file is a post-bootstrap snapshot, not a prerequisite resource.
This matches the observed empty save: after creating the region, a region EGB
and city EGB files appear even though no such file existed before selection.

## Consequences for OpenSCP

The minimum faithful bootstrap representation is therefore:

1. script map-entry properties and their image resource keys/groups;
2. starter path property arrays, including points, tangents, ranges, path
   entry IDs, scope, and road-connect values;
3. precreate unit/zone key plus transform tables;
4. the same environment/city transform and play-area bounds used by the game.

For the current lot/model offset bug, copying only ED coordinates is
insufficient. OpenSCP must preserve the `cSPTransform` supplied to
`CreateUnit` and the path point coordinate frame used by `CreatePathsFromProps`.
The game graphics code consumes `unit->mWorldTransform` directly when creating
models, so an origin/axis/scale mismatch at bootstrap shifts both lots and
their models together.

This also explains why script packages matter but are not the complete map:
they provide property tables, map image keys, path-entry definitions, and
model/effect rules. The concrete starter topology is assembled from those
properties into the live EcoGame state and only then persisted to EGB.
