# OpenSCP 文档与实现现状

更新时间：2026-11-08

本文是公开文档的现状索引。README 保持独立维护，不在本文档范围内。研究记录
（`docs/re/`）保留历史证据和当时的未决项；它们的结论不能自动覆盖当前代码，
需要以对应源码和测试重新核对。

## 当前实现基线

- 应用由 Vue 3 + Tauri 2 + Rust workspace 组成；前端命令和桌面入口位于
  `src-tauri/src/lib.rs`，前端构建与检查命令位于 `package.json`。
- 资产构建 M1 的当前处理器版本是 `openscp.property.m1/39`，定义在
  `src-tauri/src/asset_build_m1/plan.rs`。
- M1 已实现 LOTM/GLB 模型编译、场景写回、菜单注册、行为绑定和确定性 DBPF
  输出；构建阶段在 `src-tauri/src/asset_build_m1/stages.rs` 注册。
- 构建期契约包括 `layout-contract`、`menu-contract`、`delta-audit`、
  `material-binding-contract` 以及参照 lot 主体检查。
- 模型形状缺省值已经固定为 `nh-atlas`（`src-tauri/src/asset_build_m0/ir.rs`）。
- 工作台的 Mod Flow 已支持 v2 manifest、依赖图校验、内容寻址 schema、输入指纹、
  确定性地图 overlay 以及构建报告；实现位于 `src-tauri/src/mod_flow.rs` 和
  `src-tauri/src/workspace.rs`。

## 文档分区

| 路径 | 用途 | 维护口径 |
|---|---|---|
| `docs/roadmap/` | 目标、边界和里程碑 | 实现变化后更新当前状态段 |
| `docs/design/` | 尚未完全落地的设计和交互方案 | 显式标注设计、已实现和未实现边界 |
| `docs/overview/` | 格式与引擎知识 | 结论以源码、探针或包样本证据为准 |
| `docs/blog/` | 面向公开读者的已确认研究 | 不把推测写成运行时保证 |
| `docs/re/` | 日期化逆向和构建取证 | 允许历史状态；新结论应追加证据 |
| `docs/todos/` | 待办和性能任务 | 不代表已交付能力 |

## 本次修正的陈旧内容

- `docs/mod-asset-pipeline-roadmap-20261024.md` 原先把 GLB/LOTM 编译、场景写回和
  构建闸门列为未完成；现已改为当前 M1 能力与剩余 M2/M3 范围。
- 本地 ignored 的 `docs/roadmap/asset-development.md` 和 `modding-suite.md` 仍保留
  历史计划，未纳入本次公开提交；其现状以本文的实现基线和源码为准。
- `docs/roadmap/ui.md` 原先把现有工作台当作尚未注册的 P1 草案；现已补充当前
  `/studio`、资产工作台和真实 Tauri 命令边界。
- `docs/re/menu-placement-failure-matrix-20261107.md` 是历史失败矩阵；比较其中 /38
  产物时应以本文和 `plan.rs` 的 /39 当前基线为准。

## 证据

- `package.json`
- `src-tauri/src/asset_build_m0/ir.rs`
- `src-tauri/src/asset_build_m1/plan.rs`
- `src-tauri/src/asset_build_m1/stages.rs`
- `src-tauri/src/mod_flow.rs`
- `src-tauri/src/workspace.rs`
