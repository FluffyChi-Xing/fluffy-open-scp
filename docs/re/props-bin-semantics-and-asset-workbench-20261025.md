# Props bin 语义与 Assets Mod 工作台调研（2026-10-25）

## Props bin 结论

`bin` 不是确定性业务语义。lot property 使用 `0x0C12EF20 + bin` 表示 14 个 Prop Slots（bin 0–13），引擎通过该箱的资源列表、transform/slot 表和可选的资源存量驱动显示数量。真正的模型语义来自：

`lot prop bin → resourceId → EcoGame script resource → graphics key → model / impostor / sim graphics`

因此：

- 同一个 bin 数字不能稳定命名为“车辆”或“建筑”。
- 同一个语义类别也可能出现在不同 bin。
- 没有 resourceId 或脚本资源表解析结果时，只能显示“其他道具 / 未解析”，不能猜测。
- 建筑模型不应进入 props 组件库；建筑应通过 building asset / LOD 节点进入。
- 树、车辆、垃圾、街道家具等分类可以使用已验证的 resourceId 映射和名称回退，但 UI 应标注这是资源语义，不是 bin 语义。

证据来源：`docs/re/unit-props-effects-spawners-paths.md` §1、`docs/re/props-effects-spawners-paths-engine-flow.md` §1.2.1，以及 `src/pages/packages/components/property-editor/assetCategories.ts` 的已验证 resourceId 映射。

## 组件库分组规则

- Lot、decal、effect、spawner、light 作为一级页签。
- 打开的资源模型按所属 package 分组；包名来自 `packagePath`。
- Light 按 Point / Spot / Line 类型分组。
- Props 按解析出的资源语义分组，并排除 buildings；没有解析结果的项进入其他模型。
- bin 只作为 unit 身份和编辑字段保留，不作为用户可见分类标题。

## Mod Studio 面板重构

`/studio/asset` 之前仍是 `PanelStub`。本次新增 `AssetStudioWorkbench`，采用项目已有的 `Resizable`、`ResizablePanel`、`ResizableHandle`（Reka/shadcn splitter 封装）实现三栏基础：

1. 左侧资产节点树：来源、建筑、六槽材质、Lot、菜单入口。
2. 中央合成画布：节点关系和未来 Property Viewer / Editor 嵌入位置。
3. 右侧属性检查器：当前选择、绑定来源和验证状态。

下一步可把真实 Assets Mod schema、节点编辑器和 Property Editor 的 `mode="editor"` 接到中央画布；画布节点只需挂载 Property Viewer 的 canvas 插槽即可保持轻量。

