# Coding Conventions

## Core Sections (Required)

### 1) Naming Rules

| Item | Rule | Example | Evidence |
|---|---|---|---|
| Rust files | snake_case | `asset_build_m1/stages.rs` | `src-tauri/src` |
| Vue components | PascalCase | `MenuBindingForm.vue` | `src/pages` |
| Functions | snake/camel by language | `plan_from_schema`, `assembleGameUiHtml` | source files |
| Types | Rust PascalCase / TS PascalCase | `PropertyBuildPlan` | source files |

### 2) Formatting and Linting

- Formatter: rustfmt and Prettier (`prettier.config.json`).
- Linter: ESLint (`eslint.config.js`); Rust warnings are checked by Cargo tooling.
- Run: `pnpm lint`, `pnpm format:check`, `pnpm check`, `cargo test --workspace`.

### 3) Import and Module Conventions

- Rust imports are module-local and explicit; Vue uses relative aliases defined by TypeScript/Vite config.
- Public Rust APIs are exposed through crate/module exports; frontend commands are registered centrally in `lib.rs`.

### 4) Error and Logging Conventions

- Pure parsing/build functions return `Result`; Tauri commands convert failures into command errors.
- Build contracts fail closed and include stage context in `stages.rs`.
- Sensitive-data redaction: `[TODO]`; no secret-bearing integration was verified.

### 5) Testing Conventions

- Rust tests are co-located or under crate `tests`; Vue tests use `*.test.ts` beside feature code.
- Deterministic fixtures and ignored integration tests are used for local game assets.
- Coverage threshold: `[TODO]`.

### 6) Evidence

- `eslint.config.js`
- `prettier.config.json`
- `src-tauri/src/asset_build_m1/tests.rs`
