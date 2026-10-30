# Prop 语义渲染解耦：证据底表与处理器分解（2026-10-31）

日期：2026-10-31。背景：树渲染的 alpha 启发式裁剪误伤车辆漫反射（车壳被
裁穿呈黑色），已回退。用户指令：prop 渲染必须按确定语义分流成独立处理器，
禁止贴图启发式猜测。本文是证据底表与分解设计。

## 一、引擎证据：bin ≠ 物体类型

来源：docs/re/unit-props-effects-spawners-paths.md（dev 反编译取证）+
crates/sc-properties/src/lot_unit.rs（PROP_BINS = 14）。

1. **lot prop 列族 = `PROP_ID_BASE + bin`（bin 0..13，共 14 箱）**。bin 是
   **资源驱动的绘制组**：每个 BinDrawSet 绑定 EcoResourceBin，可见槽位数 =
   资源 mAmount（垃圾桶/煤堆/原油罐随资源存量显隐）；树类静态 prop 不绑
   资源 bin（全显）。**bin 编码「何时画几个」，不编码「这是什么」**。
2. **物体语义在 prop 脚本资源上**，引擎消费分叉（cGraphicsResource::
   FillFromProps，952181 行）即分类分支：

| 引擎分支（= ResolvedPropModel.source） | 属性/结构 | 语义 |
|---|---|---|
| `explicit` | 0x0D8C29C3 Key | 显式模型 key |
| `vehicle_models` | 0x0D897169 Vehicle Models key 数组 | **车辆**（引擎自带车辆通道） |
| `self-key` 族 | 0x00F9EFBB / 0x0D897169 / 0x0C36D30D 任一 | 资源自身 key 即模型 |
| `tree`/`tree_model`/`descriptor` | G 40002D00 descriptor + 0x0E0B99FD 变体数组 | **树**（引擎权威结构） |
| `lod1_direct` / `direct_model` | lot LOD key / 直挂 0x2F4E681B | 直挂模型（工作台产物） |

3. **0x08A16042（类别绑定）是死胡同**：零售包普查（prop_category_binding_
   census，2026-10-31）4987 个绑定全部为 00000000、2076 个无绑定——dev 版
   可能有值，零售无语义。
4. **未挖尽的语义矿**：GlassBox 脚本类名（cUnitPropGarbage / PropTree 等，
   EcoGame 脚本包 ER2 文本规则可读，仓库已有 erz 解析器）——垃圾桶/牌子/
   工厂产品等脚本级语义的下一 Mining 目标。

## 二、前端数据通道现状（已贯通）

- 场景文档 `units[].bin` → scene.rs 写回 property bin 列 → 查看器
  `LotUnitDto.bin`（tauri.ts:597）——**bin 通道全链路存在**，但工作台场景
  目前全放 bin 1，且 bin 语义 per-building（资源组），不全局等价物体类型。
- `ResolvedPropModel.source`（解析分支）已随 resolve_prop_models 下发——
  **分发键现成**。
- 树权威集 `treeIds`（descriptor/变体）已在查看器 context。

## 三、渲染处理器分解（设计）

分发键 = **source（引擎解析分支）**，bin 只做显隐/数量语义（模拟耦合，
查看器全显）。处理器挂载到现有 renderers/ 分发器：

| 处理器 | 分发条件 | 材质策略 |
|---|---|---|
| treeRenderer（已有） | source ∈ {tree, tree_model, descriptor} 或 treeIds 命中 | 图集/公告板 + alphaTest 0.5（自有） |
| vehicleRenderer（新） | source == vehicle_models | tint 调色链（车漆），**禁止 alphaTest**，玻璃走混合 |
| staticRenderer（= 现 propRenderer 收敛） | 其余（explicit/self-key/lod1_direct/direct_model） | 不透明漫反射，**无 alphaTest**（回退已实施） |
| signRenderer（后续） | 脚本类名/牌子资产目录 | 平面卡片 + alphaTest（牌面镂空） |

**已知遗留**：打包 lot 的直挂树（工作台场景树，property 无 modelTgi 承载）
source = direct_model → 落 staticRenderer → 叶卡白底回归。修复路径（M2）：
打包时把工作台 assetCategory（组件库已知 trees 类）写入 property 语义通道
（bin 专用通道或 lot 扩展），查看器按「场景语义 > source 分支」两级分发。
工作台编辑器内不受影响（文档 modelTgi + assetCategory 直接可用）。

## 四、已实施

- 回退 propModels.ts 通用路径的 applyAlphaCutout 接线（车辆恢复；
  three-cutout.ts 库与测试保留，归入树渲染器专用）。
- 普查探针：prop_category_binding_census（0x08A16042 死胡同证据）。
- 本文档。
