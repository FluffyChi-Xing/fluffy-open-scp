# Mod development flow (initial engine contract)

OpenSCP separates community file browsing from engine projects. A manifest activates
the Flow workbench only when `format = "openscp.mod"`, `manifest_version = 2`,
`origin = "openscp"`, and `engine = { "enabled": true, "version": 1 }`.
Older inventories with `generator: "openscp"` do not activate the engine.

New projects store description, author, mod type, and creation time. Community
projects without a manifest receive an inventory marked `origin: "community"` and
`type: "nonstandard"`. Override detection compares actual resource TGIs with the
configured game packages; `unavailable` and `partial` must not be interpreted as
“no overrides”. Existing custom fields survive inventory updates. Explicitly
enabling Flow keeps the original manifest in `legacy_metadata`.

## Persistent contract

- `nodes`: stable node IDs, kinds, schema paths, and canvas positions.
- `internal_dependencies`: node ID → prerequisite node IDs. Every node must reach
  the sole output; missing nodes, self references, duplicate edges and cycles block builds.
- `outer_dependencies`: source package/group or source package/TGI declarations,
  synchronized when schemas are saved. The build record lists exact emitted TGIs.
- Node schemas: `schema_version`, `id`, `kind`, `config`.
- Saved schema revisions use content-addressed filenames. Replacing `package.json`
  is the atomic activation point. Old revisions remain available for Git/history.
- An inspected IM contains version, manifest/schema revision, topological order,
  schemas, and SHA-256 fingerprints of source inputs. Tauri IPC transports this IM;
  the Rust engine reconstructs it and rejects stale or altered plans.

The `code_flow` command accepts `inspect`, `initialize`, `save-node`, `save-graph`,
and `build`. All project paths use the same confined workspace root as file browsing.
Assets are project-relative; source game map packages are read-only external inputs.

## Workbench

All selection controls use the shared FDropdown-backed selection component;
ESLint rejects native `select` elements. Source nodes expose only an output handle,
and the build output exposes only an input handle, enforced by backend validation.

Projects use type-specific folders, created idempotently on creation or opening:

| Mod type | Schemas | Resource folders |
| --- | --- | --- |
| Map | `schemas/map` | `assets/maps/{heightmaps,resources,roads,vegetation,water,metadata}` |
| Code | `schemas/code` | `scripts`, `assets/config` |
| Assets | `schemas/assets` | `assets/{models,textures,materials,audio}` |
| Gameplay | `schemas/gameplay` | `assets/{properties,tuning,localization}` |

All types reserve `build` for outputs. Existing files and schema references stay
valid; subsequent schema saves use the typed schema folder. `resource_layout` in
the manifest records these locations. Community imports are not reorganized until
explicitly converted to an OpenSCP project.

Community mods retain ordinary previews. Engine projects use Vue Flow; selecting a
schema or associated asset focuses its node. ComfyUI-style map cards contain their
parameter forms and the production 3D map preview; dedicated Sheet editors remain
available for complex asset work. Cards can collapse and the toolbar can arrange
them. Independent unsaved drafts survive other node saves and block building until
saved. Source nodes select a package and enumerate its regions. Dragging nodes
persists positions; connecting handles adds dependencies;
double-clicking an edge removes it. Validation explains incomplete nodes. Build is
disabled until the inspected plan is valid, with independent server-side validation.

## Implemented build capabilities

- Static resource files plus explicit TGI → deterministic DBPF 3 overlay.
- Existing region → height offset → all 341 height tiles regenerated across five
  levels from a 4096² field.
- Seeded Perlin height generation for an existing region's replacement heightfield.
- Source slot/resource/road/selection metadata preservation (no writes).
- Output stored in `build/<content-hash>/<filename>.package`; adjacent `build.json`
  records IM, input fingerprints, artifact SHA-256, and resource TGIs. Every output
  payload is reopened and compared before success is reported. No game installation
  or source package is overwritten.

## Three map templates and current limits

1. **Original map:** source → height → slots → resources → roads → selection metadata
   → output. Preserve-only nodes allow a height-only overlay today.
2. **Whole region as one city:** source → slots → coordinate alignment → resources
   → metadata → output. Coordinate alignment deliberately blocks builds until game
   simulation bounds and transitions are verified.
3. **Noise terrain:** source → Perlin → slots → resources → roads → metadata → output.
   Incomplete metadata nodes initially block builds. Explicitly preserving existing
   data produces replacement terrain, not an independently registered new map.

Slot addition/removal, resource brushes, road editing, new map registration and full
region simulation are not implemented writers. Code/gameplay types currently share
the static-resource packaging path; they do not imply a script compiler. These are
extension points, not successful no-op build operations. Source evidence and future
node decomposition remain in `re/map-editor-flow-requirements.md`.

## Verification

Backend tests cover DBPF payload roundtrip, input/schema staleness, graph validity,
manifest preservation, path traversal, actual override detection and seeded noise.
An opt-in `retail_map_flow_builds_all_height_tiles` test uses `SC_FLOW_MAP_PATH` to
verify Titan Gorge's 341-tile build and all 4096² height samples. It requires local
game files and does not add proprietary assets to the repository.

The retail integration test additionally compares the entire tile layout with
region `BEAF0510`. It exposed two issues in the previous writer: accumulated mip
filter error and ambiguous content matches between flat tiles. The writer now
checks adjacent source levels and uses the complete retail LOD identity relation
`SHARED_TILE_GRID[y][x] ^ (4 - level)` when that resource set is present. Custom
synthetic IDs retain the content-matching path. Tests cover both paths, including
an entirely flat region where content cannot identify tile positions.
