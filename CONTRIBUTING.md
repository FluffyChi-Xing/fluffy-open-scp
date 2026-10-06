# 贡献指南 / Contributing Guide

感谢你对 OpenSCP 的关注！本文档分中英文两部分，内容一致。
Thanks for your interest in contributing to OpenSCP! This guide is provided in Chinese and English; both sections are identical in content.

---

## 中文

### 分支模型与 PR 流程

- `master`：稳定主干，始终保持可构建、可打包状态。**已开启分支保护：禁止直接 push，所有变更一律通过 Pull Request 合入。**
- `dev/xx`（如 `dev/map`、`dev/rendering-optimisation`）：功能开发分支，**由项目所有者创建**，作为长期特性的集成线。
- **外部贡献者**：fork 本仓库 → 从 `master` 切出 `feat/xxx` / `fix/xxx` 分支 → 开发完成后向 `master` 发起 PR。
- **审合规则**：PR 由项目所有者审阅合并；合并前 PR 模板中的自查项需实际执行并通过。逆向结论类变更请在 PR 描述中给出证据链（`docs/roadmap/migration.md` 章节或探针输出）。
- **决策机制**：项目方向、格式底座 API 与发布节奏由项目所有者最终决策；大功能欢迎先开 Issue/Discussion 讨论达成共识再动手。
- **社区规范**：参与本项目即表示同意[行为准则](./CODE_OF_CONDUCT.md)；安全问题请勿公开 Issue，见[安全策略](./SECURITY.md)。

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
- **`tmp/` 是动态提取资源与探针处理文件的约定存放区：git 不追踪，且任何清理操作
  不得清理 `tmp/`。**逆向取证中间产物（shader dump、变体对象等）可能只存在于
  `tmp/`，删除后只能重跑探针恢复（2026-09-30 曾因整目录清理丢失 §65.15 取证材料）。
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

### Branch Model & PR Workflow

- `master`: stable trunk; always keep it buildable and shippable. **Branch protection is enabled: direct pushes are blocked — every change lands via Pull Request.**
- `dev/xx` (e.g. `dev/map`, `dev/rendering-optimisation`): feature branches, **created by the project owner**, serving as integration lines for long-running features.
- **External contributors**: fork this repo → branch `feat/xxx` / `fix/xxx` off `master` → open a PR against `master` when ready.
- **Review & merge**: PRs are reviewed and merged by the project owner; the PR-template checklist must be actually executed and green before merge. For reverse-engineering changes, cite the evidence chain in the PR description (a `docs/roadmap/migration.md` section or probe output).
- **Decision making**: project direction, foundation-crate APIs, and release cadence are ultimately decided by the project owner; large features should reach consensus in an Issue/Discussion first.
- **Community standards**: participating means agreeing to the [Code of Conduct](./CODE_OF_CONDUCT.md); never report security issues publicly — see the [Security Policy](./SECURITY.md).

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
- **`tmp/` is the designated home for dynamically extracted resources and probe
  artifacts: never tracked by git, and never deleted by any cleanup pass.**
  RE forensics (shader dumps, variant objects) may exist only inside `tmp/`;
  deleting them means re-running the probes (a 2026-09-30 cleanup wiped the
  §65.15 forensics this way).
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
