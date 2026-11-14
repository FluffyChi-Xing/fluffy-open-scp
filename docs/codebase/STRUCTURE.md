# Codebase Structure

## Core Sections (Required)

### 1) Top-Level Map

| Path | Purpose | Evidence |
|---|---|---|
| `src/` | Vue application and pages | `src/main.ts`, `src/router` |
| `src-tauri/` | Tauri commands and Rust application layer | `src-tauri/src/lib.rs` |
| `crates/` | reusable DBPF, RW4, property and export crates | `Cargo.toml` |
| `docs/` | public, design, roadmap and research docs | repository tree |
| `tools/`, `probes/` | asset/research utilities | repository tree |

### 2) Entry Points

- Main runtime entry: `src/main.ts` and `src-tauri/src/lib.rs`.
- Secondary entry points: crate examples and probe binaries under `crates/*/examples` and `probes/`.
- Selection: Vite serves the frontend; Tauri invokes registered commands in `lib.rs`.

### 3) Module Boundaries

| Boundary | What belongs here | Must not be here |
|---|---|---|
| `crates/` | format parsing, encoding and pure domain logic | Vue/Tauri UI state |
| `src-tauri/src/` | IPC, filesystem orchestration and app services | direct frontend rendering |
| `src/pages/` | route-level UI and workflow state | binary format parsing |

### 4) Naming and Organization Rules

- Rust modules use snake_case; Vue components use PascalCase; composables use `use*.ts`.
- Frontend code is organized by page/feature under `src/pages` and shared code under `src/lib`/`src/components`.
- Evidence: `src-tauri/src`, `src/pages`, `src/lib`.

### 5) Evidence

- `Cargo.toml`
- `src/main.ts`
- `src-tauri/src/lib.rs`
