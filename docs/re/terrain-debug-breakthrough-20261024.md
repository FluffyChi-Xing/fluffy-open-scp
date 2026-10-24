# 地图 debug 工具突破与游戏侧验证边界

日期：2026-10-24。

本文记录对 Reddit 所说“游戏运行时向地图添加地下资源”的复核结果。目标是把菜单、区域 brush、Terrain Editor、typed height map 和 341 资源写回链路逐段对拍，避免把残留的 Property 目录误判成可执行工具。

## 已确认的执行链

开发样本和零售反编译都保留以下调用关系：

```text
cTerrainRegion::Init
  -> cTerrainBrushes::Init
  -> cTerrainBrushes::Update
  -> RunScriptBrushes
  -> runTerrainScript visual effect
  -> cTerrainBrushEffect
  -> cTerrainEditor::ApplyBrush / ApplyGlobalBrush / ApplyRibbonBrush
  -> EndAccumulatingBrushes
  -> Save341HeightMaps
```

`RunScriptBrushes` 从 brush property list 读取：

| Property | 用途 |
| --- | --- |
| `0x02A907B5` | 主脚本 effect ID 数组（开发样本常量 `44631989`） |
| `0x02A907B6` | effect transform 数组 |
| `0x03A23F97` | effect 时间数组 |
| `0x03DD8E59` | effect seed 数组 |
| `0x03DD8E5A` | effect intensity 数组 |
| `0x0DBA3A9C` | height-map basename |
| `0x0DC097E3` | final resolution |
| `0x0DE43899` | typed height-map type |

其中后三项直接决定输出的是 terrain、water table、oil、ore 等哪一张 typed map。`Save341HeightMaps` 再将层合成为 mip 与 8192 tile，并用区域 341 key 写回资源。

## 零售包的关键事实（修订）

隐藏 Property `00B1B104:09878A01:454041B4` 是 Terrain Brush 选择器目录，包含 222 个名字，覆盖：

```text
brush_add
brush_add_global
feature_hills_*
erosion_*
brush_region_<region>_<terrain|water|forest|oil|ore|coal>
box_deposit1xV1 / box_deposit2xV1 / box_deposit3xV1
```

这些条目的 `0x0D79C8ED` Key 全部是 `type=0, group=0` 的裸 instance。对安装目录全部 package 逐条扫描后，222 个 instance 的命中数都是 0。它们是目录中的名字和 ID，占位信息，不是可以直接被 Property Manager 读取的资源。

区域描述中同样保留了 `waterTableEcoMapBrushes` 等字段。`cTerrainRegion::Init` 的真实逻辑不是直接把这些字段当作 brush 描述，而是：

1. 从区域 property 的 `0x02A907B5` 取 brush instance；
2. 以“该 instance + 当前 region group”向 Property Manager 请求另一个 property list；
3. `cTerrainBrushes::Update` 在 property list 首次变更时调用 `RunScriptBrushes`。

对 `SimCity_RegionTerrain0.package` 和 `SimCity_RegionTerrain1.package` 的全部区域字段执行闭包审计后，发现之前的“definition_count=0”结论只适用于把 child key 当作 DBPF Property TGI 查找的错误路径。真实路径是 Swarm collection 的导出哈希：

```text
waterTableEcoMapBrushes (Property 0x02A907B5)
  -> child instance hash
  -> 0xEA5118B0 export directory
  -> visual effect MetaParticle (type 2)
  -> child component type 32
  -> cTerrainBrushEffect
```

零售 `SimCity_App.package` 的三个质量集合共解析出 **519 个 typed-map export（173 × 3）**；每个 export 都能落到 `type 32`，且 `type 32` section 为 version 3、259 个可解码的 `cTerrainBrushDescription`。这证明地下水、土壤、森林和资源刷子的执行描述仍在发行版资源中，缺口不是 brush effect 本身。

菜单 action 也不能绕过这个缺口：`454041B4` 的 action 是 `0x2A9AA359`，而零售 `SC::CreateTool` factory 只处理 `cToolPlop`、`cToolPathSelector`、`cToolRegion`、`cToolTornado` 等固定 ID，没有这个 case。`CreateTools` 对该 action 会得到空 tool，因此把菜单 Parent 或 slot 改成可见分类不会产生可执行地形工具。

## 这对“游戏侧验证”的含义

### 实机范围修正（2026-10-24）

首轮 overlay 实机测试在白水谷新建城市后没有看到地下水变化。对拍
`cEcoMap::Create`、`cTerrainBrushes::Update` 和
`cTerrainExporter::RegionEcoMapsCachedOnDisk` 后，确认这不是一个合适的
“城市地下水由无变有”测试：

- `cEcoMap::Create` 会从脚本 map-entry 的图像资源初始化城市地下水
  typed map（当前静态城市图层为 `0F92DC56`），这一步独立于
  `waterTableEcoMapBrushes`。
- `waterTableEcoMapBrushes` 进入 `RunScriptBrushes`，设置
  `kEcoMapWaterTableHeightMapType`，并累积 Terrain Editor 的区域层；其
  输出属于 341 张区域 eco-map/terrain export（包括
  `2k_terrain_waterTable`），不是直接替换城市 bootstrap 图像。
- 发行版导出器在发现 `RegionEcoMapsCachedOnDisk` 已有 mip4 的 341 资源时，
  会清除 edited flag 并跳过导出。仅创建同一区域的新城市不会清空这份区域
  缓存。

因此，比较两个城市存档的“城市地下水”或 OpenSCP 的静态城市资源面板，不能
证明这个 brush overlay 是否执行。正确的验证对象应是隔离测试区域中
`0x03E421ED` 的 341 区域 eco-map tile（重点是地下水通道），并在清除/重建
区域 GPU 缓存后比较 tile/mip。若目标是让新城市的地下水从无变有，需要修改
城市 bootstrap 的 `0F92DC56` 图像/typed map 资源及其绑定链，不能只替换
`waterTableEcoMapBrushes`。

现在可以安全验证的是“区域 Property + Swarm export + type 32 + RunScriptBrushes”闭环；不能把隐藏菜单单独当作验证入口。如果只覆盖 `454041B4`：

- 菜单可能因为没有 native factory 而完全不出现；
- 即使出现，也没有 `CreateTool` 对应实现，点击后不会进入 Terrain Editor；
- 直接伪造一个空 property list 可能触发 null effect 或产生没有输出的假阳性，因此不生成这种包。

实际验证入口现在可以使用一个**单区域、单字段 overlay**，满足以下闭包条件：

```text
brush selector/menu
  + brush property list（region group）
  + `0xEA5118B0` export directory
  + `cTerrainBrushEffect` 的 type-32 description
  + brush texture/kernel 资源
  + typed map basename/resolution/type
  + Save341HeightMaps 输出资源
```

已加入最小实验生成器 [terrain_water_table_validation.rs](../../probes/terrain/terrain_water_table_validation.rs)。它只替换一个区域 `waterTableEcoMapBrushes` 的 child export（示例目标 `BEAF0510:AE71A653`，`watertableheightmap`，分辨率 128，map type 2），保留 transform、强度、时间和其它字段不变。生成的 overlay 放入游戏的包加载目录后，应在该区域新建城市，保存并重启，再比较水位面 341 eco-map tile/mip；只有重启后仍有差异，才算游戏侧写回链闭合。

## 本轮工具与证据

- `probes/terrain/terrain_brush_resource_audit.rs`：读取 `454041B4` 的刷子目录，沿 `MetaParticle → type 32` 扫描所有安装 package 的真实 Swarm 闭包，并解码 type-32 description。
- `probes/terrain/terrain_water_table_validation.rs`：生成单区域 water-table child export 替换 overlay。
- `tmp/dynamic/source-tree-impostor/src/SC/RunScriptBrushes.c`：脚本 brush 到 Terrain Editor 的调用证据。
- `tmp/dev-sample-analysis/SimCity.unwrapped.exe.c`：`cTerrainRegion::Init`、`cTerrainBrushes::Update`、`CreateTool`、`Save341HeightMaps` 的反编译证据。

## 下一步

1. 用生成器对一个新城市做 water-table round-trip，记录保存前后 341 tile/mip 的差异。
2. 对照同一 property 的 soil/forest/oil/ore 字段，确认替换 water-table child 不会触碰其它层。
3. 若 round-trip 成功，再把 export 哈希、type-32 参数和 map-type 枚举接入 Mod Studio 的地图验证面板；菜单 `0x2A9AA359` 仍单独标为“只有 UI 残留”。

## Reddit Terrain Authoring 截图与当前发行版差异

用户提供的截图展示的是一个完整的 Terrain Authoring 工作台：镜头可以越过
citybox，底部出现 Terrain Authoring 工具栏，工具栏包含 Raise Terrain、Add
Underground Water、Size/Strength，以及 Water Table Map 的实时预览。这组画面
不是单独的 `waterTableEcoMapBrushes` 初始化 overlay，而是同时满足以下状态的
开发工作台：

```text
environment.enable = true
  -> SetRegionAuthoringActive(true)
  -> regionedit camera / renderer debug state
terrainAuthoringToolActive = true
  -> Terrain Editor special draw / wireframe / brush input
  -> ApplyBrush / ApplyGlobalBrush / ApplyRibbonBrush
  -> Save341HeightMaps
```

对当前发行版的反编译结果中，`SetTerrainAuthoringToolActive(bool)` 只有定义、
没有可定位的调用点；因此仅覆盖环境开关不能承诺截图中的底部工具栏会出现。
它仍然是一个有价值的低风险实验，因为可以单独验证第一段：相机是否切换到
`regionedit`、景深和运动模糊是否关闭，以及是否允许移动到 citybox 外。

新增的 `terrain_authoring_overlay` 探针会对指定区域只修改
`0x0E276F5D` 的一个字节，保留原始 property 顺序和 `0x8000` flags。示例：

```text
cargo run -p probes --release --example terrain_authoring_overlay -- \
  "D:\\ea-games\\simcity_offline\\SimCity：Cites of Tomorrow\\SimCityData\\SimCity_RegionTerrain0.package" \
  "D:\\ea-games\\simcity_offline\\SimCity：Cites of Tomorrow\\SimCityData\\000_terrain-authoring-BEAF0510.package" \
  BEAF0510
```

当前安装目录已生成一个 `000_terrain-authoring-BEAF0510.package`，目标区域为
`BEAF0510`（白水谷对应的区域描述之一）。实测时应只观察相机/渲染状态和
citybox 边界；即使这些变化出现，也不能把它们误判为地下水已经写回。底部画笔
栏缺失将进一步证明需要定位 UI/脚本桥对 `SetTerrainAuthoringToolActive(true)`
的调用，或在 Mod Studio 中重建该桥接层。

菜单探针现在按 locale 的具体 instance 匹配，而不是只按共享的 locale table
匹配。对当前 `SimCityData` 基础包扫描结果为 `terrain-painter menus=0`，但
`Water Table Painter`、`Soil Painter`、`Forest Painter` 等 locale 字符串仍在。
这说明发行版保留了文案和 Terrain Editor 底层资源，Terrain Authoring 菜单
Property 本身已经从基础资源链移除或位于未安装的开发包中。
