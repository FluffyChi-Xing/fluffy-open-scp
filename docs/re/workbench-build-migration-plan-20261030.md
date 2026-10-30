# 工作台构建路径迁移计划（Part 3）

日期：2026-10-30。状态：**计划，未实施**。前置已落地：M1 delta 打包
（默认）、`semantics.menu` 菜单录入、大学/商业干净包与公园菜单测试包。

## 目标

资产工作台的「构建」按钮走 M1 管线（当前构建入口 `mod_flow::build` 与
工作台未连接；本轮验证全部由 opt-in 测试驱动）。迁移分四步，每步独立可验
收，前两步不碰前端。

## M1-A · 构建命令暴露（后端，~0.5 天）

1. `asset_build_m1::commands::asset_build_m1_build` 已存在（schema 文本
   进、`M1BuildResult` 出）。补一个从工作台项目目录**派生 schema** 的纯函数：
   读 `package.json` 节点图 + `asset.lot.json`（v1 文档），产出 M1 v2 schema
   JSON（reference.lot 怎么选：用户绑定的参照 lot，或菜单模式的克隆源）。
2. schema 派生必须无隐藏状态：输入文档快照，输出 schema + 阻断诊断
   （缺参照包/缺六槽/路径不闭合在 `build_from_schema` 内已有 gate，派生层
   只做字段映射，不复制校验）。
3. 测试：两份工作台样本（大学/商业）的文档 → schema → `build_from_schema`
   端到端（复用现有 fixture）。

## M1-B · 菜单绑定 UI 契约（后端 → 前端接口，~0.5 天）

1. 工作台菜单向导（`game_assets/menu_groups.rs` 调色板子分类）与资产构建
   合流：向导产出的分类/图标/名称映射为 `semantics.menu` 字段
   （categoryInstance/menuParentInstance/title/description）。
2. 前端 `menu-export.ts` 的职责收缩为「预览/草稿」，落地产出一律走后端
   （其已知缺陷：重建 property 丢未知字段、PLOP_MENU_PARENT 硬编码）。
3. 图标：工作台上传/绘制的 PNG 进 `semantics.menu.iconPng`；自动占位图仅
   用于测试。

## M1-C · 前端构建面板（前端，~2-3 天）

1. 资产项目详情页新增「构建」区块：输出包名、`outputs.mode`（默认 delta，
   高级折叠项）、菜单绑定卡片（分类树选择器 → categoryInstance，
   标题/描述/价格/维护 → semantics.menu）。
2. 调 `asset_build_m1_build`，展示 `M1BuildResult`：条目数（delta）、
   sha256、场景摘要（单元/路径/材质计数）、`menu_instance`；产物路径入
   「交付」列表（复制到游戏目录的动作复用现有 mod_flow 的安装通道）。
3. i18n：zh-CN/en-US 全部标签、空态、错误（AGENTS.md 约定）。
4. 失败路径：`build_from_schema` 的错误字符串已带定位（路径语义、共享模板
   保护、缺 PathEntry）——按错误前缀映射到可操作提示，不堆原始文本。

## M1-D · 构建后验证回路（~1 天）

1. 构建成功后自动跑查看器验收：`read_lot_model_meshes` 打开产物包，回读
   mesh 数/材质解析（对照 LOTM 容器），差异直接报在面板上（复用 10-30 的
   diag 通道）。
2. 菜单包额外校验：新 instance 六件资源齐全、locale 表可解析、图标尺寸。
3. 「复制到 SimCityData」按钮 + 同名/同 TGI 冲突提示（列出现有包，如旧
   `_v3` 全量克隆）。

## 明确不做（本计划边界）

- 不改 `property-editor` 冻结区（FROZEN.md）；新面板平行目录。
- 不实现 `semantics.unitOverrides` / `lotBindings`（plan.rs 仍拒绝）。
- 不做 ERZ/规则编辑；公园经济行为（bundle/吸引力）等游戏实测后再补。

## 游戏实测清单（当前测试包）

1. 删除 SimCityData 旧 `大学综合实验楼_v3.package`。
2. 载入 `商业公园菜单测试.package`：公园面板出现「OpenSCP 商业公园」
   （图标/大图/名称/描述/价格 8000/维护 120）→ 放置 → 模型贴图正常。
3. 载入两个 `_clean` 包：原大学/商业建筑被替换为编译模型（same-TGI 契约）。
4. 结果回填本文档；异常按 menu-item-relations §9.2（520 步长/locale 排版）
   与 landmark checklist 验收六条排查。
