<p align="center">
  <img src="docs/assets/scp-logo.png" width="180" alt="OpenSCP logo" />
</p>

[中文](README.md) | **English**

# OpenSCP

> A SimCity (2013) `.package` resource browser and modding toolchain built with Rust + Tauri 2 + Vue 3 — browse resource structures, preview models and textures, edit property lists, export to glTF/OBJ/audio/video, and progressively build a declarative Modding Suite.

## What This Project Is / Goals

OpenSCP is a modern rewrite of the classic C#/WPF tool [SimCityPak](https://github.com/altinctrl/SimCityPak), but it aims to be more than "a rewritten browser". The goals come in three layers:

1. **Reliable format foundation (P0)**: precise parsing and fidelity-preserving write-back for the DBPF container, RefPack decompression, RenderWare4 (RW4) models/textures/skeletons/animations, and property lists. All parsers are pure Rust crates (`crates/`) with no Tauri dependency, so they can be tested and reused independently.
2. **A usable resource workbench (in progress)**: package structure tree (TGI browsing), syntax-highlighted text viewing (Shiki), RW4 3D model preview (with material channels), texture/property-list/audio/video previews, Lot editor sessions (lights/decals/props/path units), and a home dashboard (extension counts/capacity share, recognized-type coverage).
3. **Mod workflow orchestration (P1)**: a declarative `openscp.mod.toml` project, OBJ import with LOD management, dependency resolution, automatic validation, and deterministic overlay package builds. See [docs/roadmap/modding-suite.md](docs/roadmap/modding-suite.md).

## Usage

### Install & Launch

1. Download an installer (MSI / NSIS) from the releases page, or build from source (see Development below);
2. On first launch, point **Settings → Game directory** at your SimCity install location (usually the folder containing `SimCityData\`, e.g. `D:\ea-games\SimCity`). OpenSCP scans it automatically and lists every `.package`;
3. You can also double-click any `.package` in the package list to open it directly, no configuration required.

### Everyday Operations

- **Browse**: the left-hand resource tree lists packages by folder; once open, filter/search/page resources by TGI (Type/Group/Instance) — smooth even on huge packages thanks to virtualized rendering. The directory comes from the persisted game folder in Settings; open packages, page numbers and filters all survive page switches;
- **Preview**: clicking a resource auto-detects its type — models (RW4 3D viewport with materials/LODs), textures (PNG/JPG/TGA/DDS/Raster), property lists (readable key-values + editing), text (JSON/HTML/JS syntax highlighting), audio (Wwise → WAV playback), video (VP6 → MP4 playback), binary (hex view);
- **Lot editor**: open a Lot resource to inspect the ground map (LotMask four-channel quantization), the model LOD chain, and light/decal/prop units with their transform matrices;
- **Export**: models → glTF 2.0 (`.glb`, with materials/skeleton/animations) or Wavefront `.obj`; textures → PNG/JPG/TGA/DDS; property lists → JSON/TXT; audio → WAV; video → MP4;
- **Home dashboard**: extension-count share (donut chart), capacity share (bar chart) and recognized-type coverage across previously opened packages, with per-package breakdowns.

### Mod Studio

Under the "Modding / Asset Creation" nav group, entry `/studio` — panels organized along a **parse → edit → build → validate** pipeline:

| Panel | Status | Capabilities |
|---|---|---|
| **TGI override scan** | Available | Scans all package indices in the game + mod directories, groups by TGI to flag overridden resources, shows the override chain (vanilla → mod1 → mod2) with content diffs; the default directory comes from Settings, with automatic and manual scans |
| **Locale text editor** | Available | stringID → translation two-pane table with dirty marks / per-line revert / filtering; exports a **deterministic overlay package** (same TGI, uncompressed, stable ordering) — drop it into the game's mod folder to override vanilla text. This is the write-back touchstone shared by all editing panels |
| **UI preview** | Preview | Global carousel of 54 in-game screens; the `assembleGameUiHtml` pipeline rebuilds game UI windows inside an iframe sandbox using real locale strings (FNV-1 screen-name mapping) + game CSS + sampled images; the editing sheet lands in WP3 |
| Property editor / Raster paint / Asset panel | Scheduled | See `docs/roadmap/workspace-panels.md` |

Static game-UI assets (CSS/locale tables/manifests) are unpacked to `public/game-ui/` (images are gitignored for size; regenerate with `cargo run -p dbpf --example ui_assets -- <SimCityData> --extract public/game-ui`).

### ⚠️ Audio/Video Transcoding Requires Manual External Tools

Audio (Wwise → WAV) and video (VP6 → MP4) preview/export **depend on two external tools that installers do not bundle**:

| Tool | Purpose | Install (either way) |
|---|---|---|
| **vgmstream** | Wwise Vorbis audio → WAV | `winget install --id vgmstream.vgmstream -e --source winget`; or download from [GitHub Releases](https://github.com/vgmstream/vgmstream); or place it in the app folder `Tools\vgmstream\` |
| **ffmpeg** | VP6 video → MP4 | `winget install --id Gyan.FFmpeg -e --source winget`; or download from [ffmpeg.org](https://ffmpeg.org); or place it in the app folder `Tools\ffmpeg\` |

Without them, audio/video previews show the tool name and install command; everything else keeps working. Lookup order: app folder `Tools\` → system `PATH`.

## Project Layout

```
fluffy-open-scp/
├── src/                    # Vue 3 frontend (fluffy-design-pro app shell)
├── src-tauri/              # Tauri 2 app shell (command binding layer)
├── crates/
│   ├── dbpf/               # DBPF (.package) container format + RefPack decompression
│   ├── rw4/                # RenderWare4 model / texture / skeleton / animation parsing
│   ├── sc-properties/      # Property list resource (0x00b1b104) parsing
│   ├── sc-registry/        # Read-only access to the database_main.s3db descriptor DB
│   ├── sc-store/           # App activity/history persistence (SQLite)
│   └── sc-exporter/        # OBJ / glTF / texture / property-list exporters
└── docs/
    ├── overview/           # Domain knowledge: rendering pipeline, file formats
    ├── rendering.md        # Renderer source research (shader reverse engineering)
    └── roadmap/            # Migration & Modding Suite planning
```

## Roadmap

- **P0 foundation** (mostly done): reliable parsing and write-back for DBPF/RefPack, RW4 and property lists; OBJ import, overlay packages, precise errors and roundtrip tests
- **Mod Studio delivered**: TGI override scan (WP0), Locale editing + overlay write-back pipeline (WP2), game-UI reconstruction preview (WP3 preview); every panel's status is honestly labeled on the `/studio` hub
- **In progress**: Property editor (WP1), UI editing (layout JSON → assembly pipeline), preview polish (model viewport material channels, day-night cycle / powered-state rendering)
- **P1 Modding Suite**: declarative `openscp.mod.toml` projects, one-click builds, LOD/dependency management, automatic validation, actionable diagnostics, preview + file watching
- **P2/P3**: advanced material/LOD/skeleton editing, game integration, dependency ecosystem and plugin capabilities

Detailed planning: [docs/roadmap/modding-suite.md](docs/roadmap/modding-suite.md); foundation migration progress: [docs/roadmap/migration.md](docs/roadmap/migration.md); domain knowledge entries: [docs/overview/rendering-pipeline.md](docs/overview/rendering-pipeline.md), [docs/overview/file-formats.md](docs/overview/file-formats.md), [docs/overview/ui-rendering-and-development.md](docs/overview/ui-rendering-and-development.md), [docs/overview/glassbox-engine.md](docs/overview/glassbox-engine.md).

## Development

Prerequisites: Rust (stable, edition 2024), Node.js ≥ 20, pnpm ≥ 11, plus the usual Tauri 2 Windows prerequisites (WebView2, MSVC Build Tools).

```bash
pnpm install          # install frontend dependencies
pnpm tauri dev        # desktop app in dev mode
pnpm tauri build      # production build
pnpm test             # frontend tests (vitest)
cargo test            # Rust tests (workspace)
```

Browser demo mode: open the Vite dev server outside Tauri, run
`localStorage.setItem("openscp:local-key", '{"version":1,"mode":"mock"}')` in the console and refresh to explore the full UI with synthetic data.

## Credits

- [altinctrl/SimCityPak](https://github.com/altinctrl/SimCityPak) — the origin of this migration (originally a CodePlex project; all original rights belong to their authors)
- [vgmstream](https://github.com/vgmstream/vgmstream) / [ffmpeg](https://ffmpeg.org) — audio/video decoding
- [fluffy-design-pro](https://github.com/FluffyChi-Xing/fluffy-design-pro) — UI app shell and component ecosystem

## License

MIT. All original rights related to SimCity and SimCityPak belong to their respective authors.
