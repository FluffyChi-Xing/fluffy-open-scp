# 贡献指南 / Contributing Guide

感谢你对 OpenSCP 的关注！本文档分中英文两部分，内容一致。
Thanks for your interest in contributing to OpenSCP! This guide is provided in Chinese and English; both sections are identical in content.

---

## 中文

### 分支模型

- `master`：稳定主干，始终保持可构建、可打包状态。
- `dev/xx`（如 `dev/map`、`dev/lots`）：功能开发分支，**由项目所有者创建**。
- 贡献流程：在所有者创建的 `dev/xx` 分支上开发并提交；功能完成后由所有者审阅合入 `master`。请勿直接向 `master` 推送大改动。

### 提交规范

使用 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)：

```
<type>(<scope>): <描述>
```

| type | 用途 |
| --- | --- |
| `feat` | 新功能 |
| `fix` | 缺陷修复 |
| `docs` | 仅文档 |
| `refactor` | 重构（无功能/修复） |
| `perf` | 性能优化 |
| `test` | 测试 |
| `build` / `ci` | 构建系统 / CI |
| `chore` | 杂项维护 |

- 描述使用祈使句、单行不超过 72 字符；一个提交只做一件逻辑上独立的事。
- `scope` 建议使用模块名：`region`（区域地图）、`map-panel`（地图面板）、`lots`（地块）、`rw4`、`dbpf` 等。

### 开发流程

```bash
# Rust（后端库 + Tauri）
cargo check -p sc-properties
cargo test  -p sc-properties --lib
cd src-tauri && cargo check

# 前端
npm run check   # vue-tsc 类型检查
npm test        # vitest
npm run dev     # 开发运行
```

提交前请确保：`vue-tsc`、`vitest`、`cargo check`、`cargo test` 全部通过。

### 探针（probes）约定

- 对游戏数据的逆向/验证结论请写成 `crates/sc-properties/examples/*.rs` 探针，
  文件头注释注明「仅开发用」与结论摘要；输出一律写入 `tmp/`（已 gitignore）。
- 沉淀到正式管线的结论请同步更新 `docs/overview/glass-box/*.md`，
  并通过 fluffy-context（`ctx learn` / `ctx checkpoint`）记录。

### 代码风格

- Rust：跟随现有代码风格，`cargo fmt` 不作强制但命名/模块组织保持一致。
- Vue/TS：`<script setup>` + Composition API；下拉一律 `FDropdown`，
  开关用 `FCheckbox`，统计图用 lieflat-charts 手写方案，禁止原生 `<select>` 与 echarts。
- 新 UI 文案必须同时补充 `src/locales/index.ts` 的 zh-CN 与 en-US 两份键。

### 数据与安全边界

- 不要提交任何游戏原版资源、`tmp/` 产物、`.context/` 或存档数据。
- 不要把本机游戏安装路径硬编码进正式管线（探针除外）。

---

## English

### Branch Model

- `master`: stable trunk; always keep it buildable and shippable.
- `dev/xx` (e.g. `dev/map`, `dev/lots`): feature branches, **created by the project owner**.
- Workflow: develop and commit on the owner-created `dev/xx` branch; the owner reviews and merges into `master`. Avoid pushing large changes directly to `master`.

### Commit Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

| type | purpose |
| --- | --- |
| `feat` | new feature |
| `fix` | bug fix |
| `docs` | docs only |
| `refactor` | refactoring (no feature/fix) |
| `perf` | performance |
| `test` | tests |
| `build` / `ci` | build system / CI |
| `chore` | maintenance |

- Imperative mood, single line under 72 chars; one logical change per commit.
- Suggested scopes: `region` (region map), `map-panel`, `lots`, `rw4`, `dbpf`.

### Development Workflow

```bash
# Rust (backend libs + Tauri)
cargo check -p sc-properties
cargo test  -p sc-properties --lib
cd src-tauri && cargo check

# Frontend
npm run check   # vue-tsc type check
npm test        # vitest
npm run dev     # dev server
```

Before committing: `vue-tsc`, `vitest`, `cargo check`, and `cargo test` must all pass.

### Probe Convention

- Reverse-engineering/verification findings for game data go into
  `crates/sc-properties/examples/*.rs` probes; mark the header with
  "dev-only" and a conclusion summary. Probe output goes to `tmp/` (gitignored).
- Promote conclusions into the official pipeline and update
  `docs/overview/glass-box/*.md`; record them via fluffy-context
  (`ctx learn` / `ctx checkpoint`).

### Code Style

- Rust: follow the existing style; keep naming and module layout consistent.
- Vue/TS: `<script setup>` + Composition API; always use `FDropdown` for dropdowns,
  `FCheckbox` for toggles, hand-rolled lieflat-charts for charts; no native
  `<select>`, no echarts.
- New UI strings must be added to both zh-CN and en-US in `src/locales/index.ts`.

### Data & Safety Boundaries

- Never commit original game assets, `tmp/` artifacts, `.context/`, or save data.
- Never hardcode local game install paths into the official pipeline (probes excepted).

---

## License / 许可证

提交即表示你同意以 [MIT License](./LICENSE) 许可你的贡献。
By submitting a contribution you agree to license your work under the [MIT License](./LICENSE).
