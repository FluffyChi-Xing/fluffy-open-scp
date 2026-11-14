# External Integrations

## Core Sections (Required)

### 1) Integration Inventory

| System | Type | Purpose | Auth model | Criticality | Evidence |
|---|---|---|---|---|---|
| SimCity package files | local file system | read game assets and write overlays | none | high | `src-tauri/src/package_service.rs` |
| SQLite s3db | local database | resolve resource names | none | medium | `crates/sc-registry` |
| vgmstream/ffmpeg | external processes | media conversion | local executable | low | `src-tauri/src` media services |

### 2) Data Stores

| Store | Role | Access layer | Key risk | Evidence |
|---|---|---|---|---|
| SQLite | app activity/registry | `sc-store`, `sc-registry` | schema migration drift | `crates/sc-store`, `crates/sc-registry` |
| DBPF packages | source/artifact store | `dbpf`, package services | wrong overlay identity | `crates/dbpf`, `asset_build_m1` |

### 3) Secrets and Credentials Handling

- Credential sources: none verified; local paths and optional tool paths are configured by the user.
- Hardcoding checks: no production credentials found in the reviewed manifests.
- Rotation: not applicable.

### 4) Reliability and Failure Behavior

- Package parsing and staged builds return typed errors and fail closed.
- External media tools have detection/fallback behavior; retry policy is `[TODO]`.
- Game runtime is intentionally not treated as an in-process integration.

### 5) Observability for Integrations

- Build stages emit reports and warnings; runtime observer commands provide diagnostic captures.
- Metrics/tracing coverage: `[TODO]`.

### 6) Evidence

- `src-tauri/src/package_service.rs`
- `src-tauri/src/asset_build_m1/stages.rs`
- `crates/dbpf/src`
