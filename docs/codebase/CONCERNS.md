# Codebase Concerns

## Core Sections (Required)

### 1) Top Risks (Prioritized)

| Severity | Concern | Evidence | Impact | Suggested action |
|---|---|---|---|---|
| high | package bytes do not prove game runtime behavior | `docs/re/menu-placement-failure-matrix-20261107.md` | false acceptance | keep cold-start and disposable-city gates |
| medium | historical docs can lag processor versions | `docs/roadmap/*`, `plan.rs` | wrong comparison baseline | update current-state index with every processor bump |
| medium | general RW4 editing remains broader than M1 compiler | `asset_build_m1/native_model.rs` | feature scope confusion | document supported input/output contract |

### 2) Technical Debt

| Debt item | Why it exists | Where | Risk if ignored | Suggested fix |
|---|---|---|---|---|
| stale dated plans | research files are append-only | `docs/re` | readers mistake history for current state | add current links and status headers |
| coverage threshold not declared | no verified threshold config | `package.json`, test config | regressions may be under-signaled | `[TODO]` define policy |

### 3) Security Concerns

| Risk | OWASP | Evidence | Current mitigation | Gap |
|---|---|---|---|---|
| local path/package input handling | A01 | workspace/package services | confined project paths and validation | broader threat model `[TODO]` |

### 4) Performance and Scaling Concerns

| Concern | Evidence | Current symptom | Scaling risk | Suggested improvement |
|---|---|---|---|---|
| large package and texture workloads | `crates/dbpf`, `crates/rw4` | memory/IO-heavy previews | slower large-package UX | retain mmap/lazy decode and measure regressions |

### 5) Fragile/High-Churn Areas

| Area | Why fragile | Churn signal | Safe change strategy |
|---|---|---|---|
| `src-tauri/src/asset_build_m1` | game contracts are byte-sensitive | recent commits and dated RE reports | add a contract test and bump processor version |
| property editor viewport | frozen/large UI surface | user directive and source size | use parallel modules and avoid frozen zones |

### 6) `[ASK USER]` Questions

1. [ASK USER] Should the current-state index become the required landing page for all public docs, or remain supplemental to README?

### 7) Evidence

- `docs/current-state.md`
- `src-tauri/src/asset_build_m1/plan.rs`
- `docs/re/menu-placement-failure-matrix-20261107.md`
