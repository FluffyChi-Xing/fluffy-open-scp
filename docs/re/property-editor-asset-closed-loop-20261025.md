# Property Editor 资产最小闭环调研（2026-10-25）

## 目标与范围

本轮只覆盖“准备好的 Blender 建筑模型绑定到游戏已有建筑资产，再通过集成菜单编辑入口进入游戏”的最小闭环。暂不纳入创建新的 lot、decal、特殊建筑规则或 ER2 新规则。

目标链路是：

`Blender GLB/LOTM → Assets Mod → Property Editor → 已有 lot/Unit/六槽材质 → Menu2 菜单入口 → package overlay → 游戏放置验证`

## 当前代码证据

- `PropertyEditor.vue` 已将场景投影为 `openscp.lot-asset/1`，支持撤销、Unit 增删改、模型预览和 JSON 导出。
- `assetFlow.ts` 已有 building4 六槽输入适配，但它是导入预览层逻辑，尚未成为 Assets Mod 的持久化契约，也没有把源文件、LOD、坐标系和单位写进资产文档。
- `usePropertyEditorSession.ts` 可以恢复已有包的 lot 单元；`modelLods` 仍是被编辑 lot 的模型引用，尚未提供“导入模型替换目标 LOD 并分配新 TGI”的正式操作。
- `uiWorkbench` 已可编辑和导出菜单，但 Menu2、`menuUnit`、Unit、lot/model 的关系仍未进入同一份资产依赖文档，无法在构建前阻止断链。
- 现有 PE 保存按钮导出 JSON 补丁，不生成可安装 DBPF；因此“导出成功”不能视为“游戏可见”。

## 本闭环必须补齐的能力

### 1. 导入契约

Assets Mod 创建后，导入资产必须记录源文件路径、格式（GLB 或 LOTM）、Z-up、米制、LOD 数量、原点/街道方向和导出工具版本。缺少这些字段时应阻止进入编译阶段。

### 2. 六槽绑定

Property Editor 需要把六个槽位作为同一建筑材质绑定显示和保存：参数表、颜色控制图、法线/AO、Shader Map、调色板、室内图集。参数表必须校验 `cols × 16`，其余槽位必须是无损 PNG；材质 shader family 不是 building4 时要明确显示“不适用”，不能静默降级。

### 3. 已有资产身份

目标建筑必须保留原始 lot、Unit 和资源 TGI 作为来源身份；新模型、材质和图标分配新 instance。Property 编辑器需要展示“来源 TGI / 新 TGI / 覆盖策略”，避免把源资源误写回或因 group/instance 冲突导致覆盖原资产。

### 4. 菜单入口依赖

菜单绑定至少需要 `Menu2 → menuUnit → Unit/Lot`，并可选关联 parent menu、icon、marquee、locale。构建前要检查这些 TGI 存在、引用方向正确、菜单类别与 Unit 类型一致；菜单编辑器的本地新增项必须能转成真实 Property overlay。

### 5. 构建与实机闸门

统一执行：静态契约校验 → 依赖闭包 → 新 TGI 分配 → RW4/Property 写回 → DBPF overlay → 重读比较 → 游戏启动/菜单/放置验证。任一阶段失败都不应生成“可安装”产物，并应生成包含源 TGI、新 TGI、菜单依赖和 SHA-256 的 manifest。

## 本次落地

新增 `assetBinding.ts` 纯校验模块，作为 Blender 导入器、Property Editor 和未来 package builder 的共享边界。它要求源文件规范化、六槽齐全且类型正确，并校验 Menu2 与 menuUnit 的 TGI 形状；对应测试覆盖完整绑定、缺槽/错误类型和非法 LOD。

## 下一步开发顺序

1. 在 Assets Mod 项目文档中持久化 `AssetBindingContract`，导入 GLB 时生成初始绑定。
2. 在 PE Schema 增加可选 `binding` 节，绑定到现有 lot/Unit 并支持新模型 TGI 预览。
3. 将 `uiWorkbench` 的 Menu2 编辑结果转换为同一依赖图，补齐 icon/locale 校验。
4. 接入 RW4/Property 写回器和 DBPF overlay，最后以真实游戏启动、菜单显示和建筑放置作为验收。

