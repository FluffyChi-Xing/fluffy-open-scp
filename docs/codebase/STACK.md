# Technology Stack

## Core Sections (Required)

### 1) Runtime Summary

| Area | Value | Evidence |
|---|---|---|
| Primary language | Rust and TypeScript/Vue | `Cargo.toml`, `package.json` |
| Runtime + version | Tauri 2 desktop runtime; Node package tooling; Rust stable/edition 2024 | `src-tauri/Cargo.toml`, `Cargo.toml`, `README.md` |
| Package manager | pnpm | `package.json`, `pnpm-lock.yaml` |
| Module/build system | Cargo workspace + Vite/Tauri | `Cargo.toml`, `vite.config.ts`, `src-tauri/tauri.conf.json` |

### 2) Production Frameworks and Dependencies

| Dependency | Role | Evidence |
|---|---|---|
| Vue 3 | UI | `package.json` |
| Tauri 2 | desktop IPC/window | `package.json`, `src-tauri/src/lib.rs` |
| Three.js | 3D previews | `package.json`, `src/` |
| rusqlite | registry/store access | `Cargo.toml`, `crates/sc-registry` |

### 3) Development Toolchain

| Tool | Purpose | Evidence |
|---|---|---|
| Vitest | frontend tests | `package.json`, `vitest.config.ts` |
| ESLint/Prettier | lint/format | `eslint.config.js`, `prettier.config.json` |
| Cargo test | Rust tests | `Cargo.toml` |

### 4) Key Commands

```text
pnpm install
pnpm check
pnpm test
pnpm build
cargo test --workspace
```

### 5) Environment and Config

- Config sources: `package.json`, `Cargo.toml`, `src-tauri/tauri.conf.json`, `.env.example`.
- Required env vars: `[TODO]` (no production secret contract verified).
- Runtime constraint: Windows Tauri/WebView2 and an optional SimCity installation for live package tests.

### 6) Evidence

- `package.json`
- `Cargo.toml`
- `src-tauri/Cargo.toml`
