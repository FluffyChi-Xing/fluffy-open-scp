# 隐藏 debug 工具与地下水资源 brush

日期：2026-10-24。

本文回答一个具体问题：Property 中出现 `waterTableEcoMapBrushes`（截图显示为地下水资源笔刷）是否意味着游戏仍保留隐藏的 debug 菜单，以及这些能力能否重新解锁。

## 结论

可以确认游戏仍保留两类能力，但它们不是同一个层次：

1. `waterTableEcoMapBrushes` 是区域静态资源的生成定义，描述地下水 typed map 如何在新区域初始化时被 stamp。它本身不是菜单项，也不能仅凭这个 Property 证明存在一个可点击的“地下水笔刷”工具。
2. 游戏发行版代码仍包含 Terrain Editor、Terrain Brush Effect、`RunScriptBrushes` 和 `kEcoMapWaterTableHeightMapType`。因此地下水 height map 的底层生成、编辑和导出链路仍在。
3. 菜单数据中确实有被隐藏的开发期 Menu 条目。至少三个已确认有可执行实现的工具，可以通过 package overlay 把 Parent 和分类改回可见菜单；其余隐藏条目不能仅凭菜单 Property 推断为可用工具。

因此，“重新解锁部分隐藏 debug 工具”是可行的；“恢复完整地下水资源编辑器 UI”目前是理论可行、实现链路尚未闭合的工作项。

## 资源层：`waterTableEcoMapBrushes` 做什么

区域描述 Property 的 brush 字段形成以下链：

```text
*EcoMapBrushes
  -> 0x02A907B5  brush key
  -> 0x02A907B6  世界坐标变换/强度
  -> 0x0DBA3A9C  目标 height-map 基名
  -> 0x0DC097E3  最终分辨率
  -> 0x0DE43899  height-map 类型索引
```

`waterTableEcoMapBrushes` 与 `coalEcoMapBrushes`、`oreEcoMapBrushes`、`oilEcoMapBrushes`、`forestEcoMapBrushes` 同属区域初始化输入。它们由区域 bootstrap/地形脚本消费，最终写入地下水等 typed map。它们不是普通的图标资源，也不是只在编辑器里显示的预览层。

直接证据：

- [map-resource-layer-facts.md](map-resource-layer-facts.md) 记录了 brush 字段、typed-map 合成和当前预览边界。
- [region-load-road-resource-lifecycle.md](region-load-road-resource-lifecycle.md) 记录了区域静态场装配和资源生成阶段。
- `probes/terrain/watertable_probe.rs` 能从区域描述中定位并解码 `waterTableEcoMapBrushes` 引用。

## 引擎层：发行版仍有 Terrain Editor 链路

`tmp/dynamic/source-tree-impostor/src/SC/RunScriptBrushes.c` 反编译出的 `SC::RunScriptBrushes` 会：

1. 创建名为 `runTerrainScript` 的离线 visual effect。
2. 从脚本 Property 读取 seed、height-map 基名、最终分辨率和 height-map 类型。
3. 调用 `TerrainEditor()->SetHeightMapType(...)`。
4. 调用 `BeginAccumulatingBrushes(...)`。
5. 运行离线 effect world，让 brush effect 执行。
6. 调用 `EndAccumulatingBrushes(...)`，结束当前类型的累积。

函数中的默认值是 `heightmap` 和 256 分辨率；Property 可以覆盖它们。反编译器对 `SetHeightMapType` 的调用少还原了一个参数，这是调用约定/反编译签名问题，不能据此认为脚本忽略了 `0x0DE43899` 的类型字段。

开发样本 `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c` 还保留了以下接口：

- `cTerrainEditor::ApplyBrush`
- `cTerrainEditor::ApplyGlobalBrush`
- `cTerrainEditor::ApplyRibbonBrush`
- `cTerrainEditor::BeginAccumulatingBrushes`
- `cTerrainEditor::EndAccumulatingBrushes`
- `cTerrainEditor::ComputeSpecificHeightMap`
- `cTerrainEditor::Save341HeightMaps`
- `cTerrainBrushEffect::ApplyBrush`
- `cTerrainBrushEffect::ApplyGlobalBrush`
- `cTerrainBrushEffect::ApplyRibbonBrush`

同一源码中的 `cTerrainExporter` 会单独导出 soil、forest、water table、oil、coal、ore 等 height map。它还使用字符串 `2k_terrain_waterTable` 保存城市地下水图。这证明地下水不是只有静态 Property 名称，运行时确实有对应的 typed map 和导出路径。

## 菜单层：确实有隐藏条目

`tmp/development-menu-survey.json` 中，父对象为 `0xBE5744E0` 的条目共有 15 个：14 个 `Menu`（类型 `0x8A01`）和 1 个 `Menu2`（类型 `0xC900`）。其中 `0xBE5744E0` 是没有完整工具属性的空壳父对象，和可见建筑工具共用的真实父对象是 `0xAF042E9A`。

已确认的隐藏 Menu instance 包括：

```text
AD947E95  A4C5D581  6B735424  A2AA4268  03650DFB
BFE9891F  C741981F  53B77423  9A2BAEE8  EB627DE3
A5800DC3  E48FCA6B  B65E1F2E  454041B4
```

其中反编译的工具 factory 明确处理了：

```text
0xC741981F -> cToolPathSelector
0x9A2BAEE8 -> cToolTornado
```

当前代码还确认了三个可通过 overlay 解锁的开发期工具：

```text
0xA2AA4268 = Edit Snap Points Tool
0x53B77423 = Edit Unit Prop Tool
0xB65E1F2E = Unit Connecter Tool
```

现有实现位于 [crates/sc-properties/src/debug_tools.rs](../../crates/sc-properties/src/debug_tools.rs)，通过逐值字节补丁把：

- debug category `0x3D752D6A` 改到可见建筑分类；
- debug UI 子分类 `0xA2A33338` 改到可见子分类；
- Parent `0xBE5744E0` 改到 `0xAF042E9A`。

写回命令位于 [src-tauri/src/debug_tools.rs](../../src-tauri/src/debug_tools.rs)。它只生成 overlay，不修改原始游戏包。

## 为什么还不能直接说“地下水 debug 菜单已经找到”

菜单条目和 Terrain Editor 的连接仍缺三段证据：

1. 哪个 Menu/Menu2 instance 的 action 或 effect 字段真正启动 `runTerrainScript`。
2. `cTerrainBrushDescription` 的资源定义如何映射到 brush key、半径、强度、随机种子和混合模式。
3. 编辑后的 typed map 如何写回区域的 341 张 tile，以及区域重新 bootstrap 时是否会覆盖它。

因此，把 `waterTableEcoMapBrushes` 直接移动到普通菜单分类，可能只得到一个无效入口，甚至会暴露只有 Property 没有 factory 实现的占位条目。解锁时必须先验证工具 factory、action/effect 引用和输出资源闭包。

## 可行性分级

| 目标 | 可行性 | 当前证据 | 推荐做法 |
| --- | --- | --- | --- |
| 显示三个已确认的开发期工具 | 已实现 | `debug_tools_write_overlay`、三个明确 instance | 先在独立 overlay 中实机验收 |
| 显示 `cToolPathSelector`、`cToolTornado` | 较高 | factory switch 有对应 case | 先补齐分类/标题/动作字段，再单独测试 |
| 显示其余隐藏 Menu 条目 | 不确定 | 有 Menu 数据，缺少 factory case 或完整属性 | 仅做只读诊断，不默认导出 |
| 直接编辑地下水 brush 并写回游戏 | 理论可行 | Terrain Editor、RunScriptBrushes、water-table type 和 exporter 均存在 | 先实现 schema/诊断闭环，再实现写回 |
| 从 Property 单独恢复完整地下水工具 | 尚未证明 | 缺菜单到脚本 action 的闭环 | 不应在 UI 中宣称已支持 |

## 下一步

1. 扫描所有 `Menu`/`Menu2` 的 action、effect、`unitEffectParameter` 和 `unitEffectTitle`，与 `runTerrainScript` 的 effect key 做反向匹配。
2. 读取 `cTerrainBrushDescription` 的资源格式，补齐 brush texture、强度、半径、global/ribbon 模式和随机种子。
3. 建立只读的 `TerrainBrushDocument`：保留源 TGI、类型、基名、分辨率、变换和原始 Property 字节。
4. 用一个 water-table 样本验证 `BeginAccumulatingBrushes → EndAccumulatingBrushes → Save341HeightMaps` 的输出命名、tile 数量和 mip 层级。
5. 只有在 round-trip 和游戏重启后仍可见的情况下，才把“地下水 brush 编辑”接入 Mod Studio 菜单。

## 证据索引

- [map-resource-layer-facts.md](map-resource-layer-facts.md)
- [region-load-road-resource-lifecycle.md](region-load-road-resource-lifecycle.md)
- [menu-mechanism-research-20261022.md](menu-mechanism-research-20261022.md)
- `tmp/dynamic/source-tree-impostor/src/SC/RunScriptBrushes.c`
- `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c`
- `tmp/development-menu-survey.json`
- `crates/sc-properties/src/debug_tools.rs`
- `src-tauri/src/debug_tools.rs`
