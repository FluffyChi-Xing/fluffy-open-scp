# Architecture

## Core Sections (Required)

### 1) Architectural Style

- Primary style: layered desktop application with pure format crates and Tauri adapters.
- Why: DBPF/RW4/property work is in `crates/`; commands and orchestration are in `src-tauri`; Vue consumes IPC DTOs.
- Constraints: no frontend binary parsing; package writes are isolated behind Rust services; build stages are explicit.

### 2) System Flow

```text
Vue page -> Tauri command -> Rust service -> format crate/package IO -> DTO or artifact
```

The command registry is in `src-tauri/src/lib.rs`; asset build lowering is in
`asset_build_m0`; staged compilation and contracts are in `asset_build_m1/stages.rs`;
DBPF and property primitives live in `crates/`.

### 3) Layer/Module Responsibilities

| Layer | Owns | Must not own | Evidence |
|---|---|---|---|
| `crates/dbpf`, `rw4`, `sc-properties` | pure format parsing/encoding | UI state | crate sources |
| `src-tauri/src` | commands, filesystem and build orchestration | Vue rendering | `lib.rs`, `asset_build_m1` |
| `src/` | views, stores and user interaction | raw DBPF parsing | `src/pages`, `src/lib` |

### 4) Reused Patterns

| Pattern | Where found | Why it exists |
|---|---|---|
| staged pipeline | `src-tauri/src/asset_build_m1/stages.rs` | fail-closed build contracts |
| pure lowering | `src-tauri/src/asset_build_m0` | deterministic validation before IO |
| command adapter | `src-tauri/src/lib.rs` | typed frontend/backend boundary |

### 5) Known Architectural Risks

- Historical docs can describe an earlier processor version; current version must be read from `plan.rs`.
- Game runtime behavior cannot be inferred from package bytes alone; live verification remains separate.

### 6) Evidence

- `src-tauri/src/lib.rs`
- `src-tauri/src/asset_build_m1/stages.rs`
- `crates/dbpf/src`
