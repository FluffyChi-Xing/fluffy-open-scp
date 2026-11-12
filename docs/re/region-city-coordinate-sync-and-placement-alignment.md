# 区域/城市坐标同步与道路、建筑放置对齐

本报告针对三个问题：区域视图进入城市时是否改变世界坐标原点；解锁或扩大边界后道路/建筑为何可能偏移；道路和建筑的定位到底使用哪些变换。结论来自 `SimCity2013-source-tree/src/SC` 与 `src/GB` 的反编译源码。源码中的函数体是静态证据，不能单独证明某个存档在运行时一定使用了某组数值。

## 结论

游戏至少维护两套相关但不同的范围：

1. `cTerrainRegion` 的区域大小和可见范围。区域默认大小与可见范围都是 `0x8000`（32768）；正式值可由 RegionTerrain 属性 `0xB0AAB5B1`（反编译显示为有符号常量 `-1330989647`）和 `0x5E3D16DF` 覆盖。
2. `GB::cIEcoGame::PlayArea()` 返回的城市可玩 AABB。道路、分区、建筑放置和大量 UI/碰撞检查都读取这个 AABB。它不是 terrain 的可见范围。

进入城市时没有发现把所有道路或建筑坐标整体平移到另一个坐标系的代码。城市地形的 `mPosInRegion` 直接取城市槽位的区域坐标，区域高度采样再把城市局部输入加回该偏移。因此保持槽位位置、城市数据和 unit/path 的世界坐标不变，只扩大 `PlayArea`，不会产生系统性平移；若编辑器同时改了槽位位置或对坐标重复加偏移，才会造成整体错位。

## 区域到城市的坐标链

`cTerrainGfx::Init`（`src/SC/cTerrainGfx/Init.c`）在区域模式下从环境属性 `0xF01DE4B1` 读取 `mPosInRegion`，并读取角度；在城市模式下读取城市索引，再通过 `cTerrainExporterCity::Init` 将 `city.mPosition` 和 `city.mDegreeAngle` 复制到 `mPosInRegion`/`mAngleInRegion`。这里没有额外的 region→city 平移。

`cTerrainRegion::Init`（`src/SC/cTerrainRegion/Init.c`）把 RegionTerrain 的尺寸转换成：

```text
halfRegionSize = regionSize / 2
cellSize       = regionSize / (1 << maxSubdivisions)
```

`cTerrainGfx::PointSampleHeightInRegion` 的关系是：

```text
regionPos.x = mPosInRegion.x + worldPos.x
regionPos.y = mPosInRegion.y + worldPos.y
```

随后用 `regionPos` 检查 `±halfRegionSize`，并按高度 tile 的 2048 网格采样。这个函数证明城市地形局部输入与区域坐标之间唯一明确的平移量就是 `mPosInRegion`；编辑器导出时不能再把城市槽位位置加一次。

区域视图的摄像机切换也遵循同一坐标来源。`cRegionCameraController::TransitionToSite` 通过游戏对象按 `GB::cSlotRef` 取得一个三维位置，把它写入摄像机 `mLerpSubject.mTarget` 和 `mRotationPoint`。该函数只改变摄像机过渡目标，没有修改 path、lot 或 terrain 数据。

## 城市盒/存档加载不负责几何平移

`cGameAppMode::LoadBoxesFromRegionInfo` 读取 JSON 的 `boxes[]`，收集每个城市的 `id`、`uid` 和 `creatorId`，随后把区域盒和城市盒交给 `GameManager::LoadBoxState`。代码维护的是 box ID、UID、owner、活动城市和 inactive city 索引；没有读取几何坐标并对城市对象做平移。

因此存档切换的坐标一致性来自盒内保存的 EcoGame/path/unit 数据与对应城市环境属性，而不是 `LoadBoxesFromRegionInfo` 在加载时临时计算的偏移。OpenSCP 若修改城市位置，必须同时更新城市槽位、引用该槽位的区域地形/高度采样关系，以及城市盒中已有 path/unit/lot 的世界坐标。

## 道路使用统一的世界坐标

`cUnitPathCreationLocal::CreatePath` 接收 `cSPVector3 points`，直接传给 `GB::cEcoPathSet::AddPoints` 和 `AddPath`；没有 region/city 偏移参数。道路距离和切线分别在 `UpdateDistanceArray`、`UpdateTangent` 中基于同一 path set 计算。

道路是否属于区域道路由脚本表中的 bitset 决定。`cGraphicsPathNetwork::Init` 从 EcoGame scripts 读取：

- `1776773985` → `mRegionPathEntries`
- `-1186731749` → `mRegionConnectionRoadPathEntries`

`IsRegionPath` 和 `cGraphicsPathPart::SetPathFlags` 将两张表合并判断区域路径。`SetPathFlags` 的 `inCity` 参数只影响渲染 flag（区域/连接道路在城市内外的 flag 值），不改变点坐标。桥和隧道也是额外 flag。

道路的边界判定统一读取 `game->PlayArea()`：

- `cGraphicsPathPart::PathInPlayArea` 对每个 segment 的两端点调用 `SP::LineIntersectsBox`；
- `IntersectionInPlayArea` 检查输入点是否落在 PlayArea AABB；
- `SC::IsSegmentInPlayArea` 对 segment 两端点逐个比较 `mMin/mMax`；
- `cZoningGame::StartGame` 用 PlayArea 的 min/max 初始化 zoning 网格。

这意味着“解锁边框”若只扩大视觉 mesh 或 `mRegionVisibleAreaAsInt`，不会放开道路/分区/建筑的可放置范围；要让新空间可用，必须改变城市 EcoGame 的 PlayArea 及其相关存档/初始化数据，并确保 path 点仍在同一世界坐标系。

## 建筑 lot 的定位链

建筑并不是把一个模型中心点直接加到城市原点。源码给出的链条是：

1. `cRegionCityLots::Init` 按 EcoGame scripts 的 `NumUnitEntries` 建立 `cUnitLotEntry`，并为实际 unit slot 创建 lot graphics。
2. `cUnitLotData::FillFromUnit` 用 unit 的 `mEntryIndex` 找到 entry，并复制 `cUnitLotEntry::GetPlacementTransform(graphicsGroup)`。
3. `cUnitLotData::CalculateUnitLotTransform` 先求 placement transform 的逆，再与 unit 的 `mWorldTransform` 做 `PreTransformBy`。这得到 lot local geometry → unit world transform 的最终变换。
4. `cRegionCityLots::CreateUnitLotGraphics` 对 bounding box、overlay corners、unit lot corners 依次应用 rotation、scale，再加 `mUnitLotTransform.mTranslation`；没有 region/city 偏移。
5. 放置阶段 `cBuildPlacementLotWalker::TestPlacement` 用 frontage/depth 构造 lot-local corners，通过 `cUnitPlacementData::GetUnitTransform` 变换到世界位置，使用 `cZoningLot::PointInLot` 检查，最后把 transform 写入 `cPlacementLotGroup::SetUnitPlacementTransform`。
6. `cZoningGame::GetUnitLotCorners` 再应用 `mUnitLotTransform`，并与 `game->PlayArea()` 比较/夹取，地面高度来自 `GroundHeight_2`。

这条链同时解释了“边框解锁后建筑不偏移”的必要条件：保持 lot local geometry、placement transform、unit world transform、frontage/depth 和 PlayArea 的世界坐标原点一致。扩大 PlayArea 只改变合法性边界，不应重算或加上城市槽位偏移。

## 对 OpenSCP 的实现建议

OpenSCP 的地图/城市编辑流程应把坐标字段分为三类，避免误写：

| 类别 | 代表来源 | 导出约束 |
|---|---|---|
| 区域坐标 | CityList `0xF01DE4B1`、`cTerrainExporterCity::mPosition` | 作为城市槽位的区域世界坐标；只写一次 |
| 城市世界坐标 | EcoPathSet 点、unit `mWorldTransform`、lot placement transform | 保持原值；移动城市时整体变换必须显式且一致 |
| 合法范围 | `GB::cIEcoGame::PlayArea()` AABB | 与可见范围独立校验；扩边不能只改渲染字段 |

建议新增导出校验：

- 检查每条 path segment 的端点、每个 lot corner 和每个 unit world transform 是否落在目标 PlayArea 或预期的跨边界白名单内；
- 检查 CityList 位置与 RegionTerrain 高度 tile 的 regionPos 映射，禁止重复应用 `mPosInRegion`；
- 对 region path/region connection road 的脚本 bitset 做一致性校验；
- 计算修改前后的锚点差值（城市槽位、道路端点、建筑 lot 中心），若出现统一非零平移则拒绝导出；
- 将 PlayArea、RegionSize、VisibleArea 分开显示并分别写入完整性摘要。

## 2026-10-30 新样本勘误：PlayArea、Eco 网格与高度 tile 是三条链

新的 retail PDB/source-tree 样本把“扩大 citybox 后坐标是否会漂移”拆成了三个可以分别验收的量：

1. `GB::cEcoGame::InitGame`（RVA `0x60BDC0`）从 `SimProperties` 的 `0x07E0AC05`
   读取 `mPlayArea`，随后用脚本 `MapBounds()` 计算 `mPlayCell`。这说明 citybox 的逻辑
   范围不是渲染网格推导出来的；边界扩展的首个写入点是 EcoGame 属性。
2. `SC::cTerrainMapSet::WorldToEco`（RVA `0x42A910`）只用
   `smScalars.mFlatScale` 把连续世界坐标映射到 `ecoSize×ecoSize` 网格，并把整数格索引
   夹到 `0..ecoSize-1`，再返回格内 `s/t`。因此“盒外”首先表现为采样网格的边缘夹取，
   不是另一套世界原点。
3. `SC::cTerrainEditor::Save341HeightMaps`（RVA `0x40D350`）导出时以
   `4096 - regionDimension/2` 为起点、每块 `8192` 世界单位，按
   `base_xNN_yNN_mipN` 生成资源名；`MakeHeightMapFileKey` 再把名字映射为
   `0x03E421F0` 高度资源 Key。高度场分块不能复用 citybox 的 PlayArea 尺寸，也不能在
   每个 tile 内重复应用 `mPosInRegion`。

编辑器应在导出前显示并校验 `PlayArea`、`mFlatScale/ecoSize`、`RegionDimension` 三组
   数值；任何统一平移只允许出现在显式的城市槽位变换中。这样可以把“边框扩大后道路/建筑
   偏移”快速归因到重复平移、PlayArea 合法性或高度 tile 分块，而不再把三种问题混为一谈。

## 仍需运行时验证的边界

源码已确认静态坐标链，但以下行为仍需用可解析样本或运行时对照确认：

- 某些版本/地图是否在 EcoGame 初始化阶段把 PlayArea 从固定 2048×2048 城市盒改成自定义 AABB；
- 跨城市连接道路在 region box 与 city box 同时加载时的 endpoint 配对和 z 高度处理；
- 边框解锁 mod 实际修改的是 PlayArea 属性、城市盒数据、还是额外的脚本合法性字段；
- 旧存档中 unit/lot transform 是否已经写成相对城市坐标，或始终是城市世界坐标。

## 2026-11-12 白水谷原包跨界对拍

样本为白水谷（Confluence，区域 group `0xBEAF0510`）。零售 `SimCity.exe` SHA-256 `2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab`，`SimCity_RegionTerrain0.package` 为 `2ff8bcac2272b83ef05fba23d3d775402c2de8b7e6d5a4383f6a22f261c6d4a6`，`SimCityDataEP1.package` 为 `1a556f499cc643e6d96a2cce8ff8492bc202e7e141c6b362ada7accd27ae758d`。通过 [region_bootstrap::load](../../crates/sc-properties/src/region_bootstrap.rs) 对 UID 和环境组后，从 site 0 读到 138 段原生样条。槽位边界采用工作台显示的 `origin ± 1024`，本次未读取运行时 `PlayArea`。每组先在原生三次样条上求过界参数 `t`，再取 `t−0.02`、`t`、`t+0.02`。

零售 EXE 的城市初始化路径在 `0xBFD47A–0xBFD489` 取 CityList 的 XY 表项，调用方在 `0xBEA38C/0xBEA39A` 把它写入地形对象 `+0x2797C/+0x27980`。区域模式的另一条路径在 `0xBEA41F` 直接把 `0xF01DE4B1` 的读取目标指向这两个字段。区域采样入口 `0xBEBFE0` 把两个字段各加一次到输入 XY，无旋转指令。另有角度字段，地形渲染等其他调用路径的旋转行为需各自核对。

零售 `0xBEB769–0xBEB7A7` 用 `trunc((regionXY + regionSize/2) / 2048)` 取区域 tile。`0xBE89E0` 从 tile 原点算像素偏移，用 `-1/8` 乘后截断，再从数据指针反向减去该偏移。对本次范围内的非负 tile 内坐标，等价于 `floor(tileLocalXY/8)`。工作台组装 F0 高度图用相同 XY 网格和 8 m 像素，ED 生态图用同一 tile 网格和 16 m 像素。以下 F0、ED instance 同时命中工作台表和按原生名字计算的 FNV1，并回读了 DBPF 像素。

| 槽位 / 样条 | 点 | 区域 XY | 城市局部 XY | Tile XY | F0 像素 / 原始 u16 | ED 像素 / BGRA |
| --- | --- | --- | --- | --- | --- | --- |
| 1026 / 16 | 盒内 | `(-516.9764, 2167.4672)` | `(1019.0236, -392.5328)` | `(7,9)` | `(191,14)` / `4263` | `(95,7)` / `253,4,255,0` |
| 1026 / 16 | 边界 | `(-512, 2171.8526)` | `(1024, -388.1474)` | `(7,9)` | `(192,15)` / `3997` | `(96,7)` / `253,3,255,0` |
| 1026 / 16 | 盒外 | `(-507.0901, 2176.2691)` | `(1028.9099, -383.7309)` | `(7,9)` | `(192,16)` / `4165` | `(96,8)` / `253,8,255,0` |
| 1027 / 19 | 盒内 | `(2034.2716, 214.0591)` | `(1010.2716, -545.9409)` | `(8,8)` | `(254,26)` / `4635` | `(127,13)` / `254,4,255,0` |
| 1027 / 19 | 边界 | `(2048, 215.0925)` | `(1024, -544.9075)` | `(9,8)` | `(0,26)` / `4735` | `(0,13)` / `254,4,255,0` |
| 1027 / 19 | 盒外 | `(2062.0480, 215.7077)` | `(1038.0480, -544.2923)` | `(9,8)` | `(1,26)` / `4753` | `(0,13)` / `254,4,255,0` |

1026 的环境组 `D41AC555`，槽位原点 `(-1536,2560)`，过界 `t≈0.683253070`，F0/ED instance 为 `B9AF73A2/6BF3934A`。1027 的环境组 `FCF2DBF8`，槽位原点 `(1024,760)`，过界 `t≈0.967923738`，F0/ED instance 从 `D8423E2C/A89FF624` 变为 `EADE327B/1D8E8193`。两段道路的 entry 为 `04F90CCD`。1027 盒外样条点 Z 为 `-863`，按 [mapToScene](../../src/components/map/map-coordinates.ts) 转成场景坐标 `(2062.0480,-863,-215.7077)`。样条 Z 与 F0 的原始 `u16` 属于不同数据层，表中未把 F0 计数当作世界高度。

1026 说明越过城市边缘可发生在同一张区域 tile 内，1027 说明也可能同时越过区域 tile 接缝。1027 盒内点在工作台城市 128² EcoMap 的 `(127,29)`，边界和盒外点按工作台半开矩形已超出城市图层。两点仍能落到区域 ED 数据上。零售高度入口在区域编辑开关关闭时对 `abs(localX/Y) > 1024` 尝试区域采样，所以边界上的 `1024` 与盒外点应分别验收。该分支还受区域 tile 缓存就绪与范围检查影响，本次仅验证机器码和原包数据，未启动游戏。
