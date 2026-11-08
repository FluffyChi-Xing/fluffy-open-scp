# Testing Patterns

## Core Sections (Required)

### 1) Test Stack and Commands

- Primary frameworks: Vitest 3 and Rust's built-in test framework.
- Commands: `pnpm test`, `pnpm check`, `cargo test --workspace`.
- Integration tests requiring local game assets are explicitly ignored or environment-gated.

### 2) Test Layout

- Rust tests are co-located in modules and in crate `tests/`; frontend tests use `*.test.ts`.
- Build fixtures and package samples live under `src-tauri/src/asset_build_m1` and `docs/packages`.

### 3) Test Scope Matrix

| Scope | Covered? | Typical target | Notes |
|---|---|---|---|
| Unit | yes | parsers, lowering, contracts | broad crate coverage |
| Integration | yes | package and build round trips | some require local assets |
| E2E | partial | Tauri/game workflows | environment-gated |

### 4) Mocking and Isolation Strategy

- Frontend browser mode uses explicit local mock storage; Rust tests use synthetic property/package fixtures.
- Real game files are read-only inputs; tests that need them are opt-in.
- Common failure: missing local game data causes gated tests to skip or fail by design.

### 5) Coverage and Quality Signals

- Coverage tool/threshold: `[TODO]`.
- Quality signals include `pnpm check`, ESLint, Vite build and workspace Cargo tests.
- Runtime behavior remains a residual gap because it needs a disposable game city.

### 6) Evidence

- `package.json`
- `vitest.config.ts`
- `src-tauri/src/asset_build_m1/tests.rs`
